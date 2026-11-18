package main

// 健康检查:定时探测每个上游的延迟与可用性,供择优与故障转移使用。

import (
	"context"
	"net/http"
	"sync"
	"time"
)

func startHealthChecks(cfg *Config, interval time.Duration) {
	check := func() {
		cfg.mu.RLock()
		ecos := cfg.Ecosystems
		cfg.mu.RUnlock()
		var wg sync.WaitGroup
		for _, e := range ecos {
			for _, up := range e.Upstreams {
				wg.Add(1)
				go func(u *Upstream) { // 并行探测,不互相阻塞
					defer wg.Done()
					u.Up, u.Latency = probe(u.Base)
				}(up)
			}
		}
		wg.Wait()
	}
	// 全部放进后台 goroutine:不阻塞引擎启动(首次探测从海外可能较慢)
	go func() {
		check()
		t := time.NewTicker(interval)
		defer t.Stop()
		for range t.C {
			check()
		}
	}()
}

// probe 返回 (可用, 延迟秒)。不可达时 Up=false,Latency=很大。
func probe(url string) (bool, float64) {
	ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodHead, url, nil)
	if err != nil {
		return false, 999
	}
	start := time.Now()
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		// HEAD 可能不被支持,退回 GET
		req2, _ := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
		resp, err = http.DefaultClient.Do(req2)
		if err != nil {
			return false, 999
		}
	}
	defer resp.Body.Close()
	lat := time.Since(start).Seconds()
	return resp.StatusCode < 500, lat
}
