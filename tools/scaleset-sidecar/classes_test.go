package main

import (
	"context"
	"errors"
	"fmt"
	"net"
	"strings"
	"testing"
)

// T-168: an error with no HTTP status names its class, and no URL, body
// or token.
func TestAnErrorWithNoStatusNamesItsClass(t *testing.T) {
	const canary = "tok-secret-77"
	cases := map[string]error{
		"sign refused: rate": fmt.Errorf("get token: sign refused: rate (url https://x.invalid/?t=%s)", canary),
		"the sign channel":   fmt.Errorf("the sign channel: %w", errors.New("broken pipe "+canary)),
		"timeout":            fmt.Errorf("post https://x.invalid/?t=%s: %w", canary, context.DeadlineExceeded),
		"dns":                fmt.Errorf("get https://x.invalid/?t=%s: %w", canary, &net.DNSError{Err: "no such host", Name: "x.invalid"}),
		"tls":                fmt.Errorf("get https://x.invalid/?t=%s: tls: failed to verify certificate: x509: unknown authority", canary),
		"connect":            fmt.Errorf("get https://x.invalid/?t=%s: %w", canary, &net.OpError{Op: "dial", Err: errors.New("connection refused")}),
		"HTTP 409":           fmt.Errorf(`request POST https://x.invalid/s?t=%s failed(status="409 Conflict"): already has an active session`, canary),
		"no answer":          errors.New("something else " + canary),
	}
	for want, err := range cases {
		got := statusOf(err)
		if got != want {
			t.Errorf("%v: %q, want %q", err, got, want)
		}
		if strings.Contains(got, canary) {
			t.Errorf("%q holds the canary", got)
		}
	}
}

type stubCloser struct{ closed int }

func (c *stubCloser) Close(context.Context) error {
	c.closed++
	return nil
}

// T-168: the message session ends at the end of stdin.
func TestTheSessionEndsAtTheEndOfStdin(t *testing.T) {
	st := newStub()
	s := stubSidecar(t, st)
	c := &stubCloser{}
	s.closer = c
	var out strings.Builder
	if code := runSidecar(s, strings.NewReader(`{"schema":1,"op":"hello","client":"p","version":"0"}`+"\n"), &out); code != 0 {
		t.Fatalf("exit %d", code)
	}
	if c.closed != 1 {
		t.Fatalf("closed %d times", c.closed)
	}
}
