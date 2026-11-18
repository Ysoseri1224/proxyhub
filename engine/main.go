package main

// ProxyHub 镜像引擎:本地路由代理。dev 工具指向它(如 pip index-url=http://127.0.0.1:8899/pypi/),
// 它按生态把请求转发到最快可用的上游镜像,失败自动换下一个,GET 响应落磁盘缓存。
// 另提供 /_ph/ 控制接口(状态/缓存/诊断)供 UI 调用。

import (
	"encoding/json"
	"io"
	"log"
	"net/http"
	"strings"
	"sync"
	"time"
)

const maxCacheObject = 200 * 1024 * 1024 // 超过 200MB 的对象只透传不缓存

type RouteEvent struct {
	Time      string `json:"time"`
	Eco       string `json:"eco"`
	Path      string `json:"path"`
	Upstream  string `json:"upstream"`
	CacheHit  bool   `json:"cacheHit"`
	Status    int    `json:"status"`
	LatencyMs int64  `json:"latencyMs"`
}

type Server struct {
	cfg    *Config
	cache  *Cache
	client *http.Client
	mu     sync.Mutex
	events []RouteEvent // 分流诊断:最近路由事件(环形)
}

func (s *Server) record(ev RouteEvent) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.events = append(s.events, ev)
	if len(s.events) > 200 {
		s.events = s.events[len(s.events)-200:]
	}
}

// ordered 在读锁保护下返回按健康排序的上游(健康字段由后台 goroutine 并发写)。
func (s *Server) ordered(e *Ecosystem) []*Upstream {
	s.cfg.mu.RLock()
	defer s.cfg.mu.RUnlock()
	return e.ordered()
}

// matchEco 最长前缀匹配生态。
func (s *Server) matchEco(path string) (*Ecosystem, string) {
	var best *Ecosystem
	for _, e := range s.cfg.Ecosystems {
		if strings.HasPrefix(path, e.Prefix) && (best == nil || len(e.Prefix) > len(best.Prefix)) {
			best = e
		}
	}
	if best == nil {
		return nil, ""
	}
	return best, strings.TrimPrefix(path, best.Prefix)
}

func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if strings.HasPrefix(r.URL.Path, "/_ph/") {
		s.control(w, r)
		return
	}
	if strings.HasPrefix(r.URL.Path, "/_fetch") {
		s.fetch(w, r)
		return
	}
	eco, rel := s.matchEco(r.URL.Path)
	if eco == nil {
		http.Error(w, "proxyhub: no ecosystem for "+r.URL.Path, http.StatusNotFound)
		return
	}
	// 注意:索引/元数据(这里的生态响应)始终回源取最新,不做磁盘缓存,
	// 以免缓存住旧索引导致"看不到新版本"。只有不可变的包文件(经 /_fetch)才缓存。

	// 按健康择优,逐个上游尝试,失败转移
	for _, up := range s.ordered(eco) {
		target := strings.TrimRight(up.Base, "/") + "/" + rel
		if r.URL.RawQuery != "" {
			target += "?" + r.URL.RawQuery
		}
		start := time.Now()
		status, ct, body, err := s.forward(r, target)
		lat := time.Since(start).Milliseconds()
		if err != nil || status >= 500 {
			continue // 换下一个上游
		}
		// 改写索引里的包文件链接,让它们也经过本地缓存
		if r.Method == http.MethodGet && status == 200 {
			selfBase := "http://127.0.0.1:" + itoa(s.cfg.Port)
			if eco.Name == "pip" && strings.Contains(ct, "html") {
				body = rewritePip(body, selfBase, target)
			} else if eco.Name == "npm" && strings.Contains(ct, "json") {
				body = rewriteNpm(body, selfBase, target)
			}
		}
		if ct != "" {
			w.Header().Set("Content-Type", ct)
		}
		w.Header().Set("X-ProxyHub-Upstream", up.Name)
		w.Header().Set("X-ProxyHub-Cache", "MISS")
		w.WriteHeader(status)
		w.Write(body)
		s.record(RouteEvent{time.Now().Format("15:04:05"), eco.Name, rel, up.Name, false, status, lat})
		return
	}
	http.Error(w, "proxyhub: all upstreams failed for "+eco.Name, http.StatusBadGateway)
	s.record(RouteEvent{time.Now().Format("15:04:05"), eco.Name, rel, "none", false, 502, 0})
}

// forward 向目标上游发起请求,返回状态、content-type、完整响应体。
func (s *Server) forward(r *http.Request, target string) (int, string, []byte, error) {
	req, err := http.NewRequest(r.Method, target, nil)
	if err != nil {
		return 0, "", nil, err
	}
	for k, vs := range r.Header {
		// 剥掉 Accept-Encoding:让 Go 自动解压,引擎始终处理明文(便于改写与正确透传)
		if k == "Host" || k == "Connection" || k == "Accept-Encoding" {
			continue
		}
		for _, v := range vs {
			req.Header.Add(k, v)
		}
	}
	resp, err := s.client.Do(req)
	if err != nil {
		return 0, "", nil, err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return 0, "", nil, err
	}
	return resp.StatusCode, resp.Header.Get("Content-Type"), body, nil
}

// control 处理 /_ph/ 控制接口。
func (s *Server) control(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	switch r.URL.Path {
	case "/_ph/status":
		type upst struct {
			Name    string  `json:"name"`
			Base    string  `json:"base"`
			Up      bool    `json:"up"`
			Latency float64 `json:"latency"`
		}
		out := map[string]any{"port": s.cfg.Port, "cacheBytes": s.cache.size(), "cacheMaxMB": s.cfg.CacheMaxMB, "ecosystems": []any{}}
		var ecos []any
		s.cfg.mu.RLock() // 与健康检查 goroutine 的写互斥
		for _, e := range s.cfg.Ecosystems {
			var ups []upst
			for _, u := range e.Upstreams {
				ups = append(ups, upst{u.Name, u.Base, u.Up, u.Latency})
			}
			ecos = append(ecos, map[string]any{"name": e.Name, "prefix": e.Prefix, "upstreams": ups})
		}
		s.cfg.mu.RUnlock()
		out["ecosystems"] = ecos
		json.NewEncoder(w).Encode(out)
	case "/_ph/diagnose":
		s.mu.Lock()
		evs := make([]RouteEvent, len(s.events))
		copy(evs, s.events)
		s.mu.Unlock()
		json.NewEncoder(w).Encode(evs)
	case "/_ph/cache/clear":
		s.cache.clear()
		json.NewEncoder(w).Encode(map[string]string{"ok": "cleared"})
	default:
		http.Error(w, "unknown control endpoint", http.StatusNotFound)
	}
}

// fetch 处理 /_fetch?u=<上游绝对URL>:带缓存地取回任意上游资源(供改写后的包文件链接使用)。
func (s *Server) fetch(w http.ResponseWriter, r *http.Request) {
	u := r.URL.Query().Get("u")
	if u == "" {
		http.Error(w, "missing u", http.StatusBadRequest)
		return
	}
	if body, ct, ok := s.cache.get("_fetch", u); ok {
		if ct != "" {
			w.Header().Set("Content-Type", ct)
		}
		w.Header().Set("X-ProxyHub-Cache", "HIT")
		w.Write(body)
		s.record(RouteEvent{time.Now().Format("15:04:05"), "_fetch", shortURL(u), "cache", true, 200, 0})
		return
	}
	start := time.Now()
	status, ct, body, err := s.forward(r, u)
	lat := time.Since(start).Milliseconds()
	if err != nil {
		http.Error(w, "fetch failed: "+err.Error(), http.StatusBadGateway)
		return
	}
	if ct != "" {
		w.Header().Set("Content-Type", ct)
	}
	w.Header().Set("X-ProxyHub-Cache", "MISS")
	w.WriteHeader(status)
	w.Write(body)
	if status == 200 && len(body) <= maxCacheObject {
		s.cache.put("_fetch", u, ct, body)
	}
	s.record(RouteEvent{time.Now().Format("15:04:05"), "_fetch", shortURL(u), "upstream", false, status, lat})
}

func shortURL(u string) string {
	if i := strings.LastIndex(u, "/"); i >= 0 && i < len(u)-1 {
		return u[i+1:]
	}
	return u
}

func main() {
	cfg, err := loadConfig()
	if err != nil {
		log.Fatalf("加载配置失败: %v", err)
	}
	s := &Server{
		cfg:    cfg,
		cache:  newCache(cfg.CacheDir, cfg.CacheMaxMB),
		client: &http.Client{Timeout: 60 * time.Second},
	}
	startHealthChecks(cfg, 60*time.Second)

	addr := "127.0.0.1:" + itoa(cfg.Port)
	log.Printf("ProxyHub 引擎启动于 http://%s  (生态: %d, 缓存: %s)", addr, len(cfg.Ecosystems), cfg.CacheDir)
	log.Printf("示例: pip config set global.index-url http://%s/pypi/", addr)
	if err := http.ListenAndServe(addr, s); err != nil {
		log.Fatal(err)
	}
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var b []byte
	for n > 0 {
		b = append([]byte{byte('0' + n%10)}, b...)
		n /= 10
	}
	return string(b)
}
