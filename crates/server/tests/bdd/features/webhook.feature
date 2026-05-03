@webhook
Feature: Webhook

  @skip
  @skip
  Scenario: Default
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    Given I have a webhook server
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                          | file   |
      | Gotenberg-Webhook-Url       | http://host.docker.internal:%d/webhook       | header |
      | Gotenberg-Webhook-Error-Url | http://host.docker.internal:%d/webhook/error | header |
    Then the response status code should be 204
    When I wait for the asynchronous request to the webhook
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the webhook request

  @skip
  @skip
  Scenario: Extra HTTP Headers
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    Given I have a webhook server
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                                | testdata/page_1.pdf                            | file   |
      | Gotenberg-Webhook-Url                | http://host.docker.internal:%d/webhook         | header |
      | Gotenberg-Webhook-Error-Url          | http://host.docker.internal:%d/webhook/error   | header |
      | Gotenberg-Webhook-Extra-Http-Headers | {"X-Foo":"bar","Content-Disposition":"inline"} | header |
    Then the response status code should be 204
    When I wait for the asynchronous request to the webhook
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then the webhook request header "X-Foo" should be "bar"
    # https://github.com/gotenberg/gotenberg/issues/1165
    Then the webhook request header "Content-Disposition" should be "inline"
    Then there should be 1 PDF(s) in the webhook request

  @skip
  @skip
  Scenario: Synchronous
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a pdfbro container with the following environment variable(s):
      | WEBHOOK_ENABLE_SYNC_MODE | true |
    Given I have a webhook server
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                          | file   |
      | Gotenberg-Webhook-Url       | http://host.docker.internal:%d/webhook       | header |
      | Gotenberg-Webhook-Error-Url | http://host.docker.internal:%d/webhook/error | header |
    Then the response status code should be 204
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the webhook request

  @skip
  @skip
  Scenario: Webhook Events URL (Success)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    Given I have a webhook server
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                        | testdata/page_1.pdf                           | file   |
      | Gotenberg-Webhook-Url        | http://host.docker.internal:%d/webhook        | header |
      | Gotenberg-Webhook-Error-Url  | http://host.docker.internal:%d/webhook/error  | header |
      | Gotenberg-Webhook-Events-Url | http://host.docker.internal:%d/webhook/events | header |
    Then the response status code should be 204
    When I wait for the asynchronous request to the webhook
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the webhook request

  @skip
  @skip
  Scenario: Webhook Events URL (Synchronous)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a pdfbro container with the following environment variable(s):
      | WEBHOOK_ENABLE_SYNC_MODE | true |
    Given I have a webhook server
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                        | testdata/page_1.pdf                           | file   |
      | Gotenberg-Webhook-Url        | http://host.docker.internal:%d/webhook        | header |
      | Gotenberg-Webhook-Error-Url  | http://host.docker.internal:%d/webhook/error  | header |
      | Gotenberg-Webhook-Events-Url | http://host.docker.internal:%d/webhook/events | header |
    Then the response status code should be 204
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the webhook request

  @skip
  @skip
  Scenario: Webhook Events URL Only (Success)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    Given I have a webhook server
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                        | testdata/page_1.pdf                           | file   |
      | Gotenberg-Webhook-Url        | http://host.docker.internal:%d/webhook        | header |
      | Gotenberg-Webhook-Events-Url | http://host.docker.internal:%d/webhook/events | header |
    Then the response status code should be 204
    When I wait for the asynchronous request to the webhook
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the webhook request

  @skip
  @skip
  Scenario: Webhook Events URL Only (Synchronous)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a pdfbro container with the following environment variable(s):
      | WEBHOOK_ENABLE_SYNC_MODE | true |
    Given I have a webhook server
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                        | testdata/page_1.pdf                           | file   |
      | Gotenberg-Webhook-Url        | http://host.docker.internal:%d/webhook        | header |
      | Gotenberg-Webhook-Events-Url | http://host.docker.internal:%d/webhook/events | header |
    Then the response status code should be 204
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the webhook request
