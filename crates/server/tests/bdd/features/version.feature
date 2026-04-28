# Feature: /version endpoint
# Ported from Gotenberg's version.feature

Feature: /version

  Scenario: GET /version
    Given I have a default Folio container
    When I make a "GET" request to "/version"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"

  Scenario: GET /version with trace header
    Given I have a default Folio container
    When I make a "GET" request to "/version"
    Then the response status code should be 200
