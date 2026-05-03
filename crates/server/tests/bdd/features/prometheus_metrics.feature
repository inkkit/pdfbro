@prometheus-metrics
Feature: /prometheus/metrics

  Scenario: GET /prometheus/metrics (Enabled)
    Given I have a pdfbro container with the following environment variable(s):
      | PROMETHEUS_DISABLE_ROUTE_TELEMETRY | false |
    When I make a "GET" request to "/prometheus/metrics"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "text/plain; version=0.0.4; charset=utf-8; escaping=underscores"

  @skip
  Scenario: GET /custom/metrics (Custom Metrics Path)
    Given I have a pdfbro container with the following environment variable(s):
      | PROMETHEUS_METRICS_PATH            | /custom/metrics |
      | PROMETHEUS_DISABLE_ROUTE_TELEMETRY | false           |
    When I make a "GET" request to "/custom/metrics"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "text/plain; version=0.0.4; charset=utf-8; escaping=underscores"

  @skip
  Scenario: GET /prometheus/metrics (Custom Namespace)
    Given I have a pdfbro container with the following environment variable(s):
      | PROMETHEUS_NAMESPACE | foo |
    When I make a "GET" request to "/prometheus/metrics"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "text/plain; version=0.0.4; charset=utf-8; escaping=underscores"

  @skip
  Scenario: GET /prometheus/metrics (Disabled)
    Given I have a pdfbro container with the following environment variable(s):
      | PROMETHEUS_DISABLE_COLLECT | true |
    When I make a "GET" request to "/prometheus/metrics"
    Then the response status code should be 404

  @skip
  Scenario: GET /prometheus/metrics (No Logging)
    Given I have a pdfbro container with the following environment variable(s):
      | PROMETHEUS_DISABLE_ROUTE_LOGGING | true |
    When I make a "GET" request to "/prometheus/metrics"
    Then the response status code should be 200

  @skip
  Scenario: GET /prometheus/metrics (Gotenberg Trace)
    Given I have a pdfbro container with the following environment variable(s):
      | PROMETHEUS_DISABLE_ROUTE_TELEMETRY | false |
    When I make a "GET" request to "/prometheus/metrics" with the following header(s):
      | Gotenberg-Trace | prometheus_metrics |
    Then the response status code should be 200
    Then the response header "Gotenberg-Trace" should be "prometheus_metrics"

  @skip
  @skip
  Scenario: GET /prometheus/metrics (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "GET" request to "/prometheus/metrics"
    Then the response status code should be 401

  @skip
  Scenario: GET /foo/prometheus/metrics (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "GET" request to "/foo/prometheus/metrics"
    Then the response status code should be 200
