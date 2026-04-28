# Feature: Webhook Async Processing
# Ported from Gotenberg's webhook.feature

Feature: Webhook

  Scenario: POST /forms/pdfengines/merge with webhook
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | page_1.pdf       | file   |
      | files                     | page_2.pdf       | file   |
      | Gotenberg-Async           | true             | header |
      | Gotenberg-Webhook-Url     | http://localhost:8080 | header |
    Then the response status code should be 202

  Scenario: POST with webhook error URL
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                      | page_1.pdf       | file   |
      | files                      | page_2.pdf       | file   |
      | Gotenberg-Async            | true             | header |
      | Gotenberg-Webhook-Url       | http://localhost:8080 | header |
      | Gotenberg-Webhook-Error-Url | http://localhost:8081 | header |
    Then the response status code should be 202
