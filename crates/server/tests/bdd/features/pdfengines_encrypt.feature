@pdfengines
@pdfengines-encrypt
@encrypt
Feature: /forms/pdfengines/encrypt

  Scenario: POST /forms/pdfengines/encrypt (default - user password only)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | userPassword | foo                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  Scenario: POST /forms/pdfengines/encrypt (default - both user and owner passwords)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files         | testdata/page_1.pdf | file  |
      | userPassword  | foo                 | field |
      | ownerPassword | bar                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  Scenario: POST /forms/pdfengines/encrypt (QPDF - user password only)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_ENCRYPT_ENGINES | qpdf |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | userPassword | foo                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  Scenario: POST /forms/pdfengines/encrypt (QPDF - both user and owner passwords)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_ENCRYPT_ENGINES | qpdf |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files         | testdata/page_1.pdf | file  |
      | userPassword  | foo                 | field |
      | ownerPassword | bar                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @skip
  Scenario: POST /forms/pdfengines/encrypt (PDFtk - user password only)
    # Reason: Folio uses lopdf/qpdf, not pdftk
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_ENCRYPT_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | userPassword | foo                 | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      pdftk: both 'userPassword' and 'ownerPassword' must be provided and different. Consider switching to another PDF engine if this behavior does not work with your workflow
      """

  @skip
  Scenario: POST /forms/pdfengines/encrypt (PDFtk - both user and owner passwords)
    # Reason: Folio uses lopdf/qpdf, not pdftk
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_ENCRYPT_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files         | testdata/page_1.pdf | file  |
      | userPassword  | foo                 | field |
      | ownerPassword | bar                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  Scenario: POST /forms/pdfengines/encrypt (pdfcpu - user password only)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_ENCRYPT_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | userPassword | foo                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  Scenario: POST /forms/pdfengines/encrypt (pdfcpu - both user and owner passwords)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_ENCRYPT_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files         | testdata/page_1.pdf | file  |
      | userPassword  | foo                 | field |
      | ownerPassword | bar                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  Scenario: POST /forms/pdfengines/encrypt (Many PDFs)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | files        | testdata/page_2.pdf | file  |
      | userPassword | foo                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  Scenario: POST /forms/pdfengines/encrypt (Bad Request)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: form field 'userPassword' is required
      """

  @skip
  Scenario: POST /forms/pdfengines/encrypt (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/encrypt (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files           | testdata/page_1.pdf      | file   |
      | userPassword    | foo                      | field  |
      | Gotenberg-Trace | forms_pdfengines_encrypt | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_encrypt"

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/encrypt (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
      | userPassword | foo                                                                                                   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/encrypt (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | userPassword                | foo                                 | field  |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/encrypt (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/encrypt (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/encrypt" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | userPassword | foo                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/pdfengines/encrypt (Long Filename)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files                     | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.pdf | file   |
      | userPassword              | foo                                                                                                                                                                                                                   | field  |
      | ownerPassword             | bar                                                                                                                                                                                                                   | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
