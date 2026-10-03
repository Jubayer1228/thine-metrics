package main

import (
	"fmt"
	"math/rand"
	"os"
	"time"

	"github.com/Jubayer1228/thine-metrics/sdks/go/thinemetrics"
)

func main() {
	endpoint := envOr("THINE_ENDPOINT", "http://localhost:4318")
	client := thinemetrics.New(endpoint)
	fmt.Println("emitting metrics to", endpoint)

	for {
		latency := 20 + rand.Float64()*100
		stats, err := client.Gauge("http.server.duration", latency, map[string]string{
			"service": "go-demo",
			"env":     "dev",
		})
		if err != nil {
			fmt.Println("error:", err)
			time.Sleep(2 * time.Second)
			continue
		}
		_, _ = client.Gauge("process.runtime.cpu.utilization", rand.Float64(), map[string]string{
			"service": "go-demo",
			"env":     "dev",
		})
		_, _ = client.Counter("http.server.request.count", float64(rand.Intn(8)+1), map[string]string{
			"service": "go-demo",
			"env":     "dev",
		})
		fmt.Printf("accepted=%d series=%d latency=%.1f\n", stats.Accepted, stats.SeriesCount, latency)
		time.Sleep(2 * time.Second)
	}
}

func envOr(k, def string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return def
}
