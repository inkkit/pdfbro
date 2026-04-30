@version
Feature: /version

  Scenario: GET /version
    Given I have a default Folio container
    When I make a "GET" request to "/version"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"

  Scenario: GET /version (Gotenberg Trace)
    Given I have a Folio container with the following environment variable(s):
      | API_DISABLE_VERSION_ROUTE_TELEMETRY | false |
    When I make a "GET" request to "/version" with the following header(s):
      | Gotenberg-Trace | version |
    Then the response status code should be 200
    Then the response header "Gotenberg-Trace" should be "version"

  Scenario: GET /version (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "GET" request to "/version"
    Then the response status code should be 401

  @folio-skip
  Scenario: GET /foo/version (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "GET" request to "/foo/version"
    Then the response status code should be 200
