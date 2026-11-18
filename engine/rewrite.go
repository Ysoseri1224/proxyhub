package main

// 把镜像索引里指向上游的"包文件链接"(含相对链接)改写成走本引擎的 /_fetch,
// 这样包文件(whl/tar.gz/tgz)也会经过本地缓存。保留 #sha256 片段以便 pip 校验。

import (
	"net/url"
	"regexp"
	"strings"
)

var reHref = regexp.MustCompile(`href="([^"]*)"`)
var reTarball = regexp.MustCompile(`("tarball"\s*:\s*")([^"]+)(")`)

// toFetch 把一个(可能相对的)链接按 base 解析为绝对,再改写为 selfBase/_fetch?u=...,保留 fragment。
func toFetch(raw, base, selfBase string) (string, bool) {
	b, err := url.Parse(base)
	if err != nil {
		return "", false
	}
	ref, err := url.Parse(raw)
	if err != nil {
		return "", false
	}
	abs := b.ResolveReference(ref)
	if abs.Scheme != "http" && abs.Scheme != "https" {
		return "", false
	}
	frag := abs.Fragment
	abs.Fragment = ""
	// 把真实文件名放进本地路径,pip/npm 才能从 URL 末段解析出包名与版本
	seg := abs.Path
	if i := strings.LastIndex(seg, "/"); i >= 0 {
		seg = seg[i+1:]
	}
	out := selfBase + "/_fetch/" + seg + "?u=" + url.QueryEscape(abs.String())
	if frag != "" {
		out += "#" + frag
	}
	return out, true
}

// rewritePip 改写 pypi simple 索引里的所有包文件链接(base = 本次请求的上游 URL)。
func rewritePip(body []byte, selfBase, upstreamURL string) []byte {
	return reHref.ReplaceAllFunc(body, func(m []byte) []byte {
		sub := reHref.FindSubmatch(m)
		if nu, ok := toFetch(string(sub[1]), upstreamURL, selfBase); ok {
			return []byte(`href="` + nu + `"`)
		}
		return m
	})
}

// rewriteNpm 改写 npm 元数据里的 tarball 链接(通常为绝对 URL)。
func rewriteNpm(body []byte, selfBase, upstreamURL string) []byte {
	return reTarball.ReplaceAllFunc(body, func(m []byte) []byte {
		sub := reTarball.FindSubmatch(m)
		if nu, ok := toFetch(string(sub[2]), upstreamURL, selfBase); ok {
			return []byte(string(sub[1]) + nu + string(sub[3]))
		}
		return m
	})
}
