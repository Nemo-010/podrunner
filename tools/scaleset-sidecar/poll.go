package main

import (
	"context"
	"sync"
	"time"

	"github.com/actions/scaleset"
)

// The long poll of GitHub holds about 50 s when no job waits. A call of
// the wire must answer in the reply limit of the mint (T-164), so the
// poll runs in its own goroutine, and `next` only takes what it found.

// messages is the part of the message session that the poller uses.
type messages interface {
	GetMessage(ctx context.Context, lastMessageID int, maxCapacity int) (*scaleset.RunnerScaleSetMessage, error)
	DeleteMessage(ctx context.Context, messageID int) error
}

const (
	pollBackoffFirst = time.Second
	pollBackoffMax   = 30 * time.Second
	// The longest wait of one `next`: below the reply limit of the mint.
	waitMax = 30 * time.Second
)

type poller struct {
	src messages
	mu  sync.Mutex
	// The capacity of the last `next`, and the one of the poll in flight.
	capacity, polling int
	// Cancels the poll in flight: a new capacity starts a new poll.
	cancel  context.CancelFunc
	lastID  int
	pending []*scaleset.RunnerScaleSetMessage
	// The error of the last poll; nil after a poll that passed.
	err   error
	ready chan struct{}
}

func newPoller(src messages) *poller {
	return &poller{src: src, ready: make(chan struct{}, 1)}
}

func (p *poller) signal() {
	select {
	case p.ready <- struct{}{}:
	default:
	}
}

// run polls until ctx ends. A message is queued only after its delete:
// a message that cannot be deleted comes again, so none is lost and none
// is read twice.
func (p *poller) run(ctx context.Context) {
	backoff := pollBackoffFirst
	for ctx.Err() == nil {
		one, cancel := context.WithCancel(ctx)
		p.mu.Lock()
		p.cancel, p.polling = cancel, p.capacity
		last, capacity := p.lastID, p.capacity
		p.mu.Unlock()
		msg, err := p.src.GetMessage(one, last, capacity)
		cancelled := one.Err() != nil && ctx.Err() == nil
		cancel()
		switch {
		case ctx.Err() != nil:
			return
		case err != nil && cancelled:
			continue // a new capacity: poll again at once
		case err != nil:
			p.mu.Lock()
			p.err = err
			p.mu.Unlock()
			p.signal()
			sleep(ctx, backoff)
			backoff = min(backoff*2, pollBackoffMax)
			continue
		}
		backoff = pollBackoffFirst
		if msg != nil {
			if derr := p.src.DeleteMessage(ctx, msg.MessageID); derr != nil {
				err = derr
				msg = nil
			}
		}
		p.mu.Lock()
		p.err = err
		if msg != nil {
			p.lastID = msg.MessageID
			p.pending = append(p.pending, msg)
		}
		p.mu.Unlock()
		if msg != nil || err != nil {
			p.signal()
		}
	}
}

// take sets the capacity of the next polls and gives the queued messages.
// With none queued it waits up to `wait` for one. With none found it
// gives the error of the last poll, if any.
func (p *poller) take(capacity int, wait time.Duration) ([]*scaleset.RunnerScaleSetMessage, error) {
	wait = min(wait, waitMax)
	p.mu.Lock()
	p.capacity = capacity
	if p.cancel != nil && p.polling != capacity {
		p.cancel()
	}
	empty := len(p.pending) == 0 && p.err == nil
	p.mu.Unlock()
	if empty && wait > 0 {
		t := time.NewTimer(wait)
		select {
		case <-p.ready:
		case <-t.C:
		}
		t.Stop()
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	got := p.pending
	p.pending = nil
	if len(got) > 0 {
		return got, nil
	}
	return nil, p.err
}

func sleep(ctx context.Context, d time.Duration) {
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-ctx.Done():
	case <-t.C:
	}
}
