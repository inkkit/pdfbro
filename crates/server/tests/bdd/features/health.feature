# Feature: Health Check Endpoint
# Ported from Gotenberg's health.feature

Feature: /health

  Scenario: GET /health
    Given I have a default Folio container
    When I make a "GET" request to "/health"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "status": "up"
      }
      """

  Scenario: GET /health with trace header
    Given I have a default Folio container
    When I make a "GET" request to "/health"
    Then the response status code should be 200

  Scenario: GET /version
    Given I have a default Folio container
    When I make a "GET" request to "/version"
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"

  Scenario: POST /forms/pdfengines/merge
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | page_1.pdf | file   |
      | files                     | page_2.pdf | file   |
      | Gotenberg-Output-Filename | result     | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | result.pdf |
    Then the "result.pdf" PDF should have 2 page(s)
