@pdfengines
@pdfengines-convert
Feature: /forms/pdfengines/convert

  Scenario: POST /forms/pdfengines/convert (Single PDF/A-1b)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfa  | PDF/A-1b            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  Scenario: POST /forms/pdfengines/convert (Single PDF/A-2b)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfa  | PDF/A-2b            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  Scenario: POST /forms/pdfengines/convert (Single PDF/A-3b)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfa  | PDF/A-3b            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  Scenario: POST /forms/pdfengines/convert (Single PDF/UA-1)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfua | true                | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/convert (Single PDF/A-1b & PDF/UA-1)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfa  | PDF/A-1b            | field |
      | pdfua | true                | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  Scenario: POST /forms/pdfengines/convert (Many PDFs)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | files | testdata/page_2.pdf | file  |
      | pdfa  | PDF/A-1b            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  Scenario: POST /forms/pdfengines/convert (Bad Request)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      either 'pdfa' or 'pdfua' form fields must be provided
      """
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | pdfa | PDF/A-1b | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Missing required file 'files'
      """
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfua | foo                 | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      either 'pdfa' or 'pdfua' form fields must be provided
      """

  @skip
  Scenario: POST /forms/pdfengines/convert (Gotenberg Trace)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files           | testdata/page_1.pdf      | file   |
      | pdfa            | PDF/A-1b                 | field  |
      | Gotenberg-Trace | forms_pdfengines_convert | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_convert"

  @output-filename
  Scenario: POST /forms/pdfengines/convert (Output Filename - Single PDF)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | pdfa                      | PDF/A-1b            | field  |
      | Gotenberg-Output-Filename | foo                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be the following file(s) in the response:
      | foo.pdf |

  @output-filename
  Scenario: POST /forms/pdfengines/convert (Output Filename - Many PDFs)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | files                     | testdata/page_2.pdf | file   |
      | pdfa                      | PDF/A-1b            | field  |
      | Gotenberg-Output-Filename | foo                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be the following file(s) in the response:
      | foo.zip    |
      | page_1.pdf |
      | page_2.pdf |

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/convert (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
      | pdfa         | PDF/A-1b                                                                                              | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/convert (Webhook)
    # Reason: pdfbro uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | pdfa                        | PDF/A-1b                            | field  |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/convert (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfa  | PDF/A-1b            | field |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/convert (Root Path)
    # Reason: pdfbro does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/convert" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | pdfa  | PDF/A-1b            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
