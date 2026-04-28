# Feature: Root endpoint
# Ported from Gotenberg's root.feature
# Note: Folio returns 404 on root (API only)

Feature: /

  Scenario: GET /
    Given I have a default Folio container
    When I make a "GET" request to "/"
    Then the response status code should be 404

  Scenario: GET /favicon.ico
    Given I have a default Folio container
    When I make a "GET" request to "/favicon.ico"
    Then the response status code should be 404
