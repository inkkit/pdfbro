@root
Feature: /

  Scenario: GET /
    Given I have a default Folio container
    When I make a "GET" request to "/"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "text/html; charset=UTF-8"

  @skip
  Scenario: GET / (Gotenberg Trace)
    Given I have a Folio container with the following environment variable(s):
      | API_DISABLE_ROOT_ROUTE_TELEMETRY | false |
    When I make a "GET" request to "/" with the following header(s):
      | Gotenberg-Trace | root |
    Then the response status code should be 200
    Then the response header "Gotenberg-Trace" should be "root"

  @skip
  @skip
  Scenario: GET / (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "GET" request to "/"
    Then the response status code should be 401

  @skip
  Scenario: GET /foo/ (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ROOT_PATH | /foo/ |
    When I make a "GET" request to "/foo/"
    Then the response status code should be 200

  Scenario: GET /favicon.ico
    Given I have a default Folio container
    When I make a "GET" request to "/favicon.ico"
    Then the response status code should be 204

  @skip
  Scenario: GET /favicon.ico (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "GET" request to "/favicon.ico" with the following header(s):
      | Gotenberg-Trace | favicon |
    Then the response status code should be 204
    Then the response header "Gotenberg-Trace" should be "favicon"

  @skip
  @skip
  Scenario: GET /favicon.ico (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "GET" request to "/favicon.ico"
    Then the response status code should be 401

  @skip
  Scenario: GET /foo/favicon.ico (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ROOT_PATH | /foo/ |
    When I make a "GET" request to "/foo/favicon.ico"
    Then the response status code should be 204
