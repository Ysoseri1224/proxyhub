package main

// 配置与默认镜像池。每个生态(pip/npm/cargo/go/brew/docker)维护一组上游镜像,按健康状况择优+故障转移。

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"sync"
)

// Upstream 一个上游镜像。
type Upstream struct {
	Name string `json:"name"`
	Base string `json:"base"` // 转发目标前缀,如 https://mirrors.aliyun.com/pypi/simple/
	// 运行时健康状态(不持久化)
	Up      bool    `json:"-"`
	Latency float64 `json:"-"` // 秒;不可达时为很大值
}

// Ecosystem 一个生态及其上游池。
type Ecosystem struct {
	Name      string      `json:"name"`
	Prefix    string      `json:"prefix"` // 本地路径前缀,如 /pypi/
	Upstreams []*Upstream `json:"upstreams"`
}

// Config 全局配置。
type Config struct {
	Port       int          `json:"port"`
	CacheDir   string       `json:"cacheDir"`
	CacheMaxMB int64        `json:"cacheMaxMB"`
	Ecosystems []*Ecosystem `json:"ecosystems"`
	mu         sync.RWMutex `json:"-"`
}

// ordered 返回按"可用优先、延迟升序"排序后的上游副本。
func (e *Ecosystem) ordered() []*Upstream {
	out := make([]*Upstream, len(e.Upstreams))
	copy(out, e.Upstreams)
	sort.SliceStable(out, func(i, j int) bool {
		if out[i].Up != out[j].Up {
			return out[i].Up // 可用的排前
		}
		return out[i].Latency < out[j].Latency
	})
	return out
}

func defaultConfig() *Config {
	home, _ := os.UserHomeDir()
	return &Config{
		Port:       8899,
		CacheDir:   filepath.Join(home, ".cache", "proxyhub"),
		CacheMaxMB: 10240, // 10 GB
		Ecosystems: []*Ecosystem{
			{Name: "pip", Prefix: "/pypi/", Upstreams: []*Upstream{
				{Name: "aliyun", Base: "https://mirrors.aliyun.com/pypi/simple/"},
				{Name: "tuna", Base: "https://pypi.tuna.tsinghua.edu.cn/simple/"},
				{Name: "tencent", Base: "https://mirrors.cloud.tencent.com/pypi/simple/"},
				{Name: "ustc", Base: "https://mirrors.ustc.edu.cn/pypi/simple/"},
			}},
			{Name: "npm", Prefix: "/npm/", Upstreams: []*Upstream{
				{Name: "npmmirror", Base: "https://registry.npmmirror.com/"},
				{Name: "tencent", Base: "https://mirrors.cloud.tencent.com/npm/"},
				{Name: "huawei", Base: "https://repo.huaweicloud.com/repository/npm/"},
			}},
			{Name: "cargo", Prefix: "/cargo/", Upstreams: []*Upstream{
				{Name: "ustc", Base: "https://mirrors.ustc.edu.cn/crates.io-index/"},
				{Name: "tuna", Base: "https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"},
				{Name: "sjtu", Base: "https://mirrors.sjtug.sjtu.edu.cn/crates.io-index/"},
			}},
			{Name: "go", Prefix: "/go/", Upstreams: []*Upstream{
				{Name: "goproxy.cn", Base: "https://goproxy.cn/"},
				{Name: "aliyun", Base: "https://mirrors.aliyun.com/goproxy/"},
				{Name: "tencent", Base: "https://goproxy.tencent.com/"},
			}},
			{Name: "brew", Prefix: "/brew/", Upstreams: []*Upstream{
				{Name: "tuna", Base: "https://mirrors.tuna.tsinghua.edu.cn/homebrew-bottles/"},
				{Name: "ustc", Base: "https://mirrors.ustc.edu.cn/homebrew-bottles/"},
				{Name: "aliyun", Base: "https://mirrors.aliyun.com/homebrew/homebrew-bottles/"},
			}},
			{Name: "docker", Prefix: "/docker/", Upstreams: []*Upstream{
				{Name: "daocloud", Base: "https://docker.m.daocloud.io/"},
				{Name: "1ms", Base: "https://docker.1ms.run/"},
				{Name: "xuanyuan", Base: "https://docker.xuanyuan.me/"},
			}},
		},
	}
}

func configPath() string {
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".config", "proxyhub", "config.json")
}

// loadConfig 读配置;不存在则写出默认配置。
func loadConfig() (*Config, error) {
	p := configPath()
	data, err := os.ReadFile(p)
	if err != nil {
		cfg := defaultConfig()
		_ = cfg.save()
		return cfg, nil
	}
	cfg := &Config{}
	if err := json.Unmarshal(data, cfg); err != nil {
		return nil, err
	}
	return cfg, nil
}

func (c *Config) save() error {
	p := configPath()
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		return err
	}
	c.mu.RLock()
	defer c.mu.RUnlock()
	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(p, data, 0o644)
}

func (c *Config) eco(name string) *Ecosystem {
	for _, e := range c.Ecosystems {
		if e.Name == name {
			return e
		}
	}
	return nil
}
