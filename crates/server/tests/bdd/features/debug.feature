@debug
Feature: /debug

  @skip
  Scenario: GET /debug (Disabled)
    # Reason: Folio has no pprof or Go debug endpoints
    Given I have a default Folio container
    When I make a "GET" request to "/debug"
    Then the response status code should be 404

  @skip
  Scenario: GET /debug (Enabled)
    # Reason: Folio has no pprof or Go debug endpoints
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true |
    When I make a "GET" request to "/debug"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"

  @skip
  Scenario: GET /debug (Environment based timezone)
    # Reason: Folio has no pprof or Go debug endpoints
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true             |
      | TZ                     | America/New_York |
    When I make a "GET" request to "/debug"
    Then the response status code should be 200

  @skip
  Scenario: GET /debug (No Debug Data)
    # Reason: Folio has no pprof or Go debug endpoints
    Given I have a Folio container with the following environment variable(s):
      | GOTENBERG_BUILD_DEBUG_DATA | false |
      | API_ENABLE_DEBUG_ROUTE     | true  |
    When I make a "GET" request to "/debug"
    Then the response status code should be 200

  @skip
  Scenario: GET /debug (Gotenberg Trace)
    # Reason: Folio has no pprof or Go debug endpoints
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE            | true  |
      | API_DISABLE_DEBUG_ROUTE_TELEMETRY | false |
    When I make a "GET" request to "/debug" with the following header(s):
      | Gotenberg-Trace | debug |
    Then the response status code should be 200
    Then the response header "Gotenberg-Trace" should be "debug"

  @skip
  @skip
  Scenario: GET /debug (Basic Auth)
    # Reason: Folio has no pprof or Go debug endpoints
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE            | true |
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "GET" request to "/debug"
    Then the response status code should be 401

  @skip
  Scenario: GET /foo/debug (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "GET" request to "/foo/debug"
    Then the response status code should be 200
