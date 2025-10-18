package docker

import (
	"bufio"
	"context"
	"strings"
	"time"

	"github.com/docker/docker/api/types/container"
)

// StreamLogs streams logs from a specific container
func (d *Client) StreamLogs(ctx context.Context, c *Container) (<-chan LogEntry, error) {
	logs := make(chan LogEntry)

	// Find the host that matches this container's host
	var targetHost *Host

	for _, host := range d.hosts {
		if host.Host == c.Host {
			targetHost = &host
			break
		}
	}

	if targetHost == nil {
		close(logs)
		return logs, nil
	}

	go func() {
		defer close(logs)

		options := container.LogsOptions{
			ShowStdout: true,
			ShowStderr: true,
			Follow:     true,
			Timestamps: true,
			Tail:       "100", // Start with last 100 lines
		}

		reader, err := targetHost.ContainerLogs(ctx, c.ID, options)
		if err != nil {
			return
		}
		defer reader.Close()

		scanner := bufio.NewScanner(reader)
		for scanner.Scan() {
			line := scanner.Text()
			if len(line) == 0 {
				continue
			}

			stream := "stdout"
			message := line

			// Skip the Docker multiplexed stream header if present
			if len(line) > 8 {
				streamType := line[0]
				switch streamType {
				case 1:
					stream = "stdout"
					message = line[8:]
				case 2:
					stream = "stderr"
					message = line[8:]
				}
			}

			// Parse timestamp from message if present
			// Docker timestamp format: 2024-01-15T10:30:45.123456789Z message text

			var timestamp time.Time
			if len(message) > 30 {
				// Look for RFC3339Nano timestamp at the start
				timestampEnd := strings.IndexByte(message, ' ')
				if timestampEnd > 0 {
					timestampStr := message[:timestampEnd]
					if parsed, err := time.Parse(time.RFC3339Nano, timestampStr); err == nil {
						timestamp = parsed
						message = message[timestampEnd+1:] // Remove timestamp from message
					}
				}
			}

			entry := LogEntry{
				ContainerID: c.ID,
				Message:     message,
				Timestamp:   timestamp,
				Stream:      stream,
			}

			select {
			case <-ctx.Done():
				return
			case logs <- entry:
			}
		}
	}()

	return logs, nil
}
