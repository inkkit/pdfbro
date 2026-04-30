@pdfengines
@pdfengines-flatten
@flatten
Feature: /forms/pdfengines/flatten

  Scenario: POST /forms/pdfengines/flatten (Single PDF)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/flatten (Many PDFs)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
      | files | testdata/page_2.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  Scenario: POST /forms/pdfengines/flatten (Bad Request)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Invalid form data: no form file found for extensions: [.pdf]
      """

  Scenario: POST /forms/pdfengines/flatten (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 404

  Scenario: POST /forms/pdfengines/flatten (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files           | testdata/page_1.pdf      | file   |
      | Gotenberg-Trace | forms_pdfengines_flatten | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_flatten"

  @output-filename
  Scenario: POST /forms/pdfengines/flatten (Output Filename - Single PDF)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | Gotenberg-Output-Filename | foo                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be the following file(s) in the response:
      | foo.pdf |

  @output-filename
  Scenario: POST /forms/pdfengines/flatten (Output Filename - Many PDFs)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | files                     | testdata/page_2.pdf | file   |
      | Gotenberg-Output-Filename | foo                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be the following file(s) in the response:
      | foo.zip    |
      | page_1.pdf |
      | page_2.pdf |

  @folio-skip
  @download-from
  Scenario: POST /forms/pdfengines/flatten (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @folio-skip
  @webhook
  Scenario: POST /forms/pdfengines/flatten (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  Scenario: POST /forms/pdfengines/flatten (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 401

  @folio-skip
  Scenario: POST /foo/forms/pdfengines/flatten (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/flatten" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/pdfengines/flatten (Long Filename)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/flatten" with the following form data and header(s):
      | files                     | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.pdf | file   |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
