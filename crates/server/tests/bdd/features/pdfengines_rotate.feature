@pdfengines
@pdfengines-rotate
@rotate
Feature: /forms/pdfengines/rotate

  Scenario: POST /forms/pdfengines/rotate (90 - All Pages - pdfcpu)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_ROTATE_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/pages_3.pdf | file  |
      | rotateAngle | 90                   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "pages_3.pdf" PDF should have 3 page(s)

  Scenario: POST /forms/pdfengines/rotate (180 - All Pages - pdfcpu)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_ROTATE_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | rotateAngle | 180                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "page_1.pdf" PDF should have 1 page(s)

  Scenario: POST /forms/pdfengines/rotate (270 - All Pages - pdfcpu)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_ROTATE_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | rotateAngle | 270                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "page_1.pdf" PDF should have 1 page(s)

  Scenario: POST /forms/pdfengines/rotate (90 - Specific Pages - pdfcpu)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_ROTATE_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/pages_3.pdf | file  |
      | rotateAngle | 90                   | field |
      | rotatePages | 1,3                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "pages_3.pdf" PDF should have 3 page(s)

  @skip
  @skip
  Scenario: POST /forms/pdfengines/rotate (90 - All Pages - pdftk)
    # Reason: pdfbro uses lopdf/qpdf, not pdftk
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_ROTATE_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | rotateAngle | 90                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "page_1.pdf" PDF should have 1 page(s)

  @skip
  @skip
  Scenario: POST /forms/pdfengines/rotate (Specific Pages - pdftk unsupported)
    # Reason: pdfbro uses lopdf/qpdf, not pdftk
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_ROTATE_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/pages_3.pdf | file  |
      | rotateAngle | 90                   | field |
      | rotatePages | 1,3                  | field |
    Then the response status code should be 500

  Scenario: POST /forms/pdfengines/rotate (Bad Request - Invalid Angle)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | rotateAngle | 45                  | field |
    Then the response status code should be 400
    Then the response body should contain string:
      """
      rotateAngle is not valid
      """

  Scenario: POST /forms/pdfengines/rotate (Bad Request - Missing Angle)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Missing required field 'rotateAngle'
      """

  Scenario: POST /forms/pdfengines/rotate (Bad Request - No PDF)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | rotateAngle | 90 | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Missing required file 'files'
      """

  Scenario: POST /forms/pdfengines/rotate (Many PDFs)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | files       | testdata/page_2.pdf | file  |
      | rotateAngle | 90                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  @skip
  Scenario: POST /forms/pdfengines/rotate (Routes Disabled)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | rotateAngle | 90                  | field |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/rotate (Gotenberg Trace)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files           | testdata/page_1.pdf     | file   |
      | rotateAngle     | 90                      | field  |
      | Gotenberg-Trace | forms_pdfengines_rotate | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_rotate"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/rotate (Webhook)
    # Reason: pdfbro uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | rotateAngle                 | 90                                  | field  |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/rotate (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | rotateAngle | 90                  | field |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/rotate (Root Path)
    # Reason: pdfbro does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/rotate" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | rotateAngle | 90                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/pdfengines/rotate (Long Filename)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/rotate" with the following form data and header(s):
      | files                     | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.pdf | file   |
      | rotateAngle               | 90                                                                                                                                                                                                                    | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
