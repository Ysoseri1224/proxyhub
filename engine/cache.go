package main

// 磁盘缓存:按请求 key 存响应体 + content-type,超过上限按最久未访问(LRU)淘汰。
// 无需任何证书——因为客户端是"显式"把本引擎当镜像端点,引擎去上游取 HTTPS 再缓存,不涉及透明劫持。

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"time"
)

type Cache struct {
	dir    string
	maxB   int64
	mu     sync.Mutex
}

func newCache(dir string, maxMB int64) *Cache {
	_ = os.MkdirAll(dir, 0o755)
	return &Cache{dir: dir, maxB: maxMB * 1024 * 1024}
}

func (c *Cache) key(eco, rel string) string {
	sum := sha256.Sum256([]byte(eco + "|" + rel))
	return hex.EncodeToString(sum[:])
}

func (c *Cache) bodyPath(k string) string { return filepath.Join(c.dir, k) }
func (c *Cache) ctPath(k string) string   { return filepath.Join(c.dir, k+".ct") }

// get 命中返回 (body, contentType, true)。命中会刷新访问时间。
func (c *Cache) get(eco, rel string) ([]byte, string, bool) {
	k := c.key(eco, rel)
	c.mu.Lock()
	defer c.mu.Unlock()
	body, err := os.ReadFile(c.bodyPath(k))
	if err != nil {
		return nil, "", false
	}
	ct, _ := os.ReadFile(c.ctPath(k))
	now := time.Now()
	_ = os.Chtimes(c.bodyPath(k), now, now) // 刷新 atime/mtime 作为 LRU 依据
	return body, string(ct), true
}

func (c *Cache) put(eco, rel, contentType string, body []byte) {
	k := c.key(eco, rel)
	c.mu.Lock()
	defer c.mu.Unlock()
	_ = os.WriteFile(c.bodyPath(k), body, 0o644)
	_ = os.WriteFile(c.ctPath(k), []byte(contentType), 0o644)
	c.evictLocked()
}

// evictLocked 超限时按 mtime 最旧优先删除,直到降到上限以下。
func (c *Cache) evictLocked() {
	type ent struct {
		path string
		size int64
		mod  time.Time
	}
	var ents []ent
	var total int64
	entries, _ := os.ReadDir(c.dir)
	for _, e := range entries {
		if e.IsDir() || filepath.Ext(e.Name()) == ".ct" {
			continue
		}
		info, err := e.Info()
		if err != nil {
			continue
		}
		ents = append(ents, ent{filepath.Join(c.dir, e.Name()), info.Size(), info.ModTime()})
		total += info.Size()
	}
	if total <= c.maxB {
		return
	}
	sort.Slice(ents, func(i, j int) bool { return ents[i].mod.Before(ents[j].mod) })
	for _, e := range ents {
		if total <= c.maxB {
			break
		}
		_ = os.Remove(e.path)
		_ = os.Remove(e.path + ".ct")
		total -= e.size
	}
}

// size 返回当前缓存总字节数。
func (c *Cache) size() int64 {
	c.mu.Lock()
	defer c.mu.Unlock()
	var total int64
	entries, _ := os.ReadDir(c.dir)
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		if info, err := e.Info(); err == nil {
			total += info.Size()
		}
	}
	return total
}

func (c *Cache) clear() {
	c.mu.Lock()
	defer c.mu.Unlock()
	entries, _ := os.ReadDir(c.dir)
	for _, e := range entries {
		_ = os.Remove(filepath.Join(c.dir, e.Name()))
	}
}
