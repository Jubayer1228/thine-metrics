// Package thinemetrics sends metrics to a Thine Metrics server.
//
// Use the JSON client for zero OTEL deps, or configure the OpenTelemetry
// Go SDK with an OTLP/HTTP exporter pointed at /v1/metrics.
package thinemetrics

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"
)

type Client struct {
	BaseURL    string
	HTTPClient *http.Client
}

func New(baseURL string) *Client {
	return &Client{
		BaseURL: strings.TrimRight(baseURL, "/"),
		HTTPClient: &http.Client{
			Timeout: 10 * time.Second,
		},
	}
}

type Point struct {
	Name        string            `json:"name"`
	Type        string            `json:"type,omitempty"`
	Value       float64           `json:"value"`
	Tags        map[string]string `json:"tags,omitempty"`
	TimestampMS int64             `json:"timestamp_ms,omitempty"`
}

type IngestStats struct {
	Accepted    uint64 `json:"accepted"`
	Rejected    uint64 `json:"rejected"`
	SeriesCount uint64 `json:"series_count"`
	SampleCount uint64 `json:"sample_count"`
}

func (c *Client) Gauge(name string, value float64, tags map[string]string) (*IngestStats, error) {
	return c.Ingest([]Point{{
		Name:        name,
		Type:        "gauge",
		Value:       value,
		Tags:        tags,
		TimestampMS: time.Now().UnixMilli(),
	}})
}

func (c *Client) Counter(name string, value float64, tags map[string]string) (*IngestStats, error) {
	return c.Ingest([]Point{{
		Name:        name,
		Type:        "counter",
		Value:       value,
		Tags:        tags,
		TimestampMS: time.Now().UnixMilli(),
	}})
}

func (c *Client) Ingest(points []Point) (*IngestStats, error) {
	body, err := json.Marshal(points)
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequest(http.MethodPost, c.BaseURL+"/api/v1/ingest", bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := c.HTTPClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		return nil, fmt.Errorf("thine ingest failed: %s", resp.Status)
	}
	var stats IngestStats
	if err := json.NewDecoder(resp.Body).Decode(&stats); err != nil {
		return nil, err
	}
	return &stats, nil
}

// OTELEnv returns environment variables that point the OTEL SDK/collector at Thine.
func OTELEnv(endpoint string) map[string]string {
	base := strings.TrimRight(endpoint, "/")
	return map[string]string{
		"OTEL_EXPORTER_OTLP_ENDPOINT":         base,
		"OTEL_EXPORTER_OTLP_PROTOCOL":         "http/json",
		"OTEL_EXPORTER_OTLP_METRICS_ENDPOINT": base + "/v1/metrics",
	}
}
