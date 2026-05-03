@health
Feature: /health

  Scenario: GET /health
    Given I have a pdfbro container with the following environment variable(s):
      | API_DISABLE_HEALTH_CHECK_ROUTE_TELEMETRY | false |
    When I make a "GET" request to "/health"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json; charset=utf-8"
    Then the response body should match JSON:
      """
      {
        "status": "up",
        "details": {
          "chromium": {
            "status": "up",
            "timestamp": "ignore"
          },
          "libreoffice": {
            "status": "up",
            "timestamp": "ignore"
          }
        }
      }
      """

  @skip
  Scenario: GET /health (No Logging)
    Given I have a pdfbro container with the following environment variable(s):
      | API_DISABLE_HEALTH_CHECK_ROUTE_TELEMETRY | true |
    When I make a "GET" request to "/health"
    Then the response status code should be 200

  @skip
  Scenario: GET /health (Gotenberg Trace)
    Given I have a pdfbro container with the following environment variable(s):
      | API_DISABLE_HEALTH_CHECK_ROUTE_TELEMETRY | false |
    When I make a "GET" request to "/health" with the following header(s):
      | Gotenberg-Trace | get_health |
    Then the response status code should be 200
    Then the response header "Gotenberg-Trace" should be "get_health"

  @skip
  @skip
  Scenario: GET /health (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "GET" request to "/health"
    Then the response status code should be 200

  @skip
  Scenario: GET /foo/health (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ROOT_PATH | /foo/ |
    When I make a "GET" request to "/foo/health"
    Then the response status code should be 200

  Scenario: HEAD /health
    Given I have a pdfbro container with the following environment variable(s):
      | API_DISABLE_HEALTH_CHECK_ROUTE_TELEMETRY | false |
    When I make a "HEAD" request to "/health"
    Then the response status code should be 200

  @skip
  Scenario: HEAD /health (Gotenberg Trace)
    Given I have a pdfbro container with the following environment variable(s):
      | API_DISABLE_HEALTH_CHECK_ROUTE_TELEMETRY | false |
    When I make a "HEAD" request to "/health" with the following header(s):
      | Gotenberg-Trace | head_health |
    Then the response status code should be 200
    Then the response header "Gotenberg-Trace" should be "head_health"

  @skip
  Scenario: HEAD /health (No Logging)
    Given I have a pdfbro container with the following environment variable(s):
      | API_DISABLE_HEALTH_CHECK_ROUTE_TELEMETRY | true |
    When I make a "HEAD" request to "/health"
    Then the response status code should be 200

  @skip
  @skip
  Scenario: HEAD /health (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "HEAD" request to "/health"
    Then the response status code should be 200

  @skip
  Scenario: HEAD /foo/health (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ROOT_PATH | /foo/ |
    When I make a "HEAD" request to "/foo/health"
    Then the response status code should be 200
