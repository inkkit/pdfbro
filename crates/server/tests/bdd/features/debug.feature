# Feature: Debug Endpoints
# Ported from Gotenberg's debug.feature

Feature: Debug Routes

  Scenario: GET /debug/vars (not enabled)
    Given I have a default Folio container
    When I make a "GET" request to "/debug/vars"
    Then the response status code should be 404

  Scenario: GET /debug/pprof (not enabled)
    Given I have a default Folio container
    When I make a "GET" request to "/debug/pprof"
    Then the response status code should be 404
