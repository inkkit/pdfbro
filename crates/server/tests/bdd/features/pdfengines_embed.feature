@pdfengines
@pdfengines-embed
@embed
Feature: /forms/pdfengines/embed

  @skip
  Scenario: POST /forms/pdfengines/embed
    # Reason: Embed file check step not yet implemented
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/embed" with the following form data and header(s):
      | files  | testdata/page_1.pdf  | file |
      | embeds | testdata/embed_1.xml | file |
      | embeds | testdata/embed_2.xml | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  Scenario: POST /forms/pdfengines/embed with metadata
    # Reason: Embed file check step not yet implemented
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/embed" with the following form data and header(s):
      | files          | testdata/page_1.pdf                                                                                                              | file  |
      | embeds         | testdata/embed_1.xml                                                                                                             | file  |
      | embeds         | testdata/embed_2.xml                                                                                                             | file  |
      | embedsMetadata | {"embed_1.xml":{"mimeType":"text/xml","relationship":"Data"},"embed_2.xml":{"mimeType":"text/xml","relationship":"Alternative"}} | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/embed with (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/embed" with the following form data and header(s):
      | files        | testdata/page_1.pdf                                                                                                                                                            | file  |
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/embed_1.xml","embedded": true},{"url":"http://host.docker.internal/static/testdata/embed_2.xml","embedded": false}]       | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/embed (Webhook)
    # Reason: pdfbro uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/embed" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | embeds                      | testdata/embed_1.xml                | file   |
      | embeds                      | testdata/embed_2.xml                | file   |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/embed (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/embed" with the following form data and header(s):
      | files  | testdata/page_1.pdf  | file |
      | embeds | testdata/embed_1.xml | file |
      | embeds | testdata/embed_2.xml | file |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/embed (Root Path)
    # Reason: pdfbro does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/embed" with the following form data and header(s):
      | files  | testdata/page_1.pdf  | file |
      | embeds | testdata/embed_1.xml | file |
      | embeds | testdata/embed_2.xml | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
