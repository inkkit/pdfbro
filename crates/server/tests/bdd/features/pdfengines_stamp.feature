@pdfengines
@pdfengines-stamp
@stamp
Feature: /forms/pdfengines/stamp

  Scenario: POST /forms/pdfengines/stamp (Text - pdfcpu)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | stampSource     | text                | field |
      | stampExpression | CONFIDENTIAL        | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "page_1.pdf" PDF should have 1 page(s)

  Scenario: POST /forms/pdfengines/stamp (Text with Pages - pdfcpu)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/pages_3.pdf | file  |
      | stampSource     | text                 | field |
      | stampExpression | DRAFT                | field |
      | stampPages      | 1-2                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "pages_3.pdf" PDF should have 3 page(s)

  Scenario: POST /forms/pdfengines/stamp (Text with Options - pdfcpu)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf                                  | file  |
      | stampSource     | text                                                 | field |
      | stampExpression | SAMPLE                                               | field |
      | stampOptions    | {"scale":"0.5 abs","rot":"45","fillcolor":"#FF0000"} | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/stamp (Image - pdfcpu)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files       | testdata/page_1.pdf    | file  |
      | stamp       | testdata/watermark.png | file  |
      | stampSource | image                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/stamp (PDF - pdfcpu)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | stamp       | testdata/page_2.pdf | file  |
      | stampSource | pdf                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  Scenario: POST /forms/pdfengines/stamp (PDF - pdftk)
    # Reason: Folio uses lopdf/qpdf, not pdftk
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | stamp       | testdata/page_2.pdf | file  |
      | stampSource | pdf                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  Scenario: POST /forms/pdfengines/stamp (Text - pdftk unsupported)
    # Reason: Folio uses lopdf/qpdf, not pdftk
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | stampSource     | text                | field |
      | stampExpression | CONFIDENTIAL        | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      At least one PDF engine cannot process the requested stamp source type, while others may have failed due to different issues
      """

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/stamp (Image via Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files        | testdata/page_1.pdf                                                                      | file  |
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/watermark.png","field":"stamp"}]    | field |
      | stampSource  | image                                                                                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/stamp (PDF via Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_STAMP_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files        | testdata/page_1.pdf                                                                   | file  |
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_2.pdf","field":"stamp"}]    | field |
      | stampSource  | pdf                                                                                   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/stamp (Many PDFs)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | files           | testdata/page_2.pdf | file  |
      | stampSource     | text                | field |
      | stampExpression | DRAFT               | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  Scenario: POST /forms/pdfengines/stamp (Bad Request - No Source)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: form field 'stampSource' is required
      """

  Scenario: POST /forms/pdfengines/stamp (Bad Request - Invalid Source)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | stampSource | foo                 | field |
    Then the response status code should be 400
    Then the response body should contain string:
      """
      Invalid form data: form field 'stampSource' is invalid
      """

  Scenario: POST /forms/pdfengines/stamp (Bad Request - Missing File for Image Source)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | stampSource | image               | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a stamp file is required for image or pdf source
      """

  Scenario: POST /forms/pdfengines/stamp (Bad Request - Missing File for PDF Source)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | stampSource | pdf                 | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a stamp file is required for image or pdf source
      """

  Scenario: POST /forms/pdfengines/stamp (Bad Request - No PDF)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | stampSource     | text         | field |
      | stampExpression | CONFIDENTIAL | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: no form file found for extensions: [.pdf]
      """

  @skip
  Scenario: POST /forms/pdfengines/stamp (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | stampSource     | text                | field |
      | stampExpression | CONFIDENTIAL        | field |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/stamp (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf    | file   |
      | stampSource     | text                   | field  |
      | stampExpression | CONFIDENTIAL           | field  |
      | Gotenberg-Trace | forms_pdfengines_stamp | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_stamp"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/stamp (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | stampSource                 | text                                | field  |
      | stampExpression             | CONFIDENTIAL                        | field  |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/stamp (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | stampSource     | text                | field |
      | stampExpression | CONFIDENTIAL        | field |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/stamp (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/stamp" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | stampSource     | text                | field |
      | stampExpression | CONFIDENTIAL        | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
