# Feature: Prometheus Metrics
# Ported from Gotenberg's prometheus_metrics.feature

Feature: /prometheus

  Scenario: GET /prometheus (not enabled by default)
    Given I have a default Folio container
    When I make a "GET" request to "/prometheus"
    Then the response status code should be 404

  Scenario: GET /metrics (not enabled by default)
    Given I have a default Folio container
    When I make a "GET" request to "/metrics"
    Then the response status code should be 404
