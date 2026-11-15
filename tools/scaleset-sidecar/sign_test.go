package main

import (
	"bufio"
	"fmt"
	"os"
	"syscall"
	"testing"
	"time"
)

// T-165: sign gives its last token again with an older exp; the sidecar
// keeps it only until that exp.
func TestATokenGivenAgainKeepsItsOwnExp(t *testing.T) {
	fds, err := syscall.Socketpair(syscall.AF_UNIX, syscall.SOCK_STREAM, 0)
	if err != nil {
		t.Fatal(err)
	}
	ours, theirs := os.NewFile(uintptr(fds[0]), "ours"), os.NewFile(uintptr(fds[1]), "theirs")
	defer ours.Close()
	defer theirs.Close()
	exp := time.Now().Unix() + 200
	go func() {
		r := bufio.NewReader(theirs)
		if _, err := r.ReadBytes('\n'); err == nil {
			fmt.Fprintf(theirs, "{\"token\":\"a.b.c\",\"exp\":%d}\n", exp)
		}
	}()
	s := newSigner(ours, "1")
	tok, err := s.token(1)
	if err != nil || tok != "a.b.c" {
		t.Fatal(tok, err)
	}
	if !s.until.Equal(time.Unix(exp, 0)) {
		t.Fatalf("kept until %v, want %v", s.until, time.Unix(exp, 0))
	}
}
