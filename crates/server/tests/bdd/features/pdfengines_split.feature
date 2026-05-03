@pdfengines
@pdfengines-split
@split
Feature: /forms/pdfengines/split

  Scenario: POST /forms/pdfengines/split (Intervals - Default)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3_0.pdf |
      | pages_3_1.pdf |
    Then the "pages_3_0.pdf" PDF should have 2 page(s)
    Then the "pages_3_1.pdf" PDF should have 1 page(s)
    Then the "pages_3_0.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "pages_3_0.pdf" PDF should have the following content at page 2:
      """
      Page 2
      """
    Then the "pages_3_1.pdf" PDF should have the following content at page 1:
      """
      Page 3
      """

  Scenario: POST /forms/pdfengines/split (Pages - Default)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | pages                | field |
      | splitSpan | 2-                   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3_0.pdf |
      | pages_3_1.pdf |
    Then the "pages_3_0.pdf" PDF should have 1 page(s)
    Then the "pages_3_1.pdf" PDF should have 1 page(s)
    Then the "pages_3_0.pdf" PDF should have the following content at page 1:
      """
      Page 2
      """
    Then the "pages_3_1.pdf" PDF should have the following content at page 1:
      """
      Page 3
      """

  Scenario: POST /forms/pdfengines/split (Pages & Unify - Default)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files      | testdata/pages_3.pdf | file  |
      | splitMode  | pages                | field |
      | splitSpan  | 2-                   | field |
      | splitUnify | true                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3.pdf |
    Then the "pages_3.pdf" PDF should have 2 page(s)
    Then the "pages_3.pdf" PDF should have the following content at page 1:
      """
      Page 2
      """
    Then the "pages_3.pdf" PDF should have the following content at page 2:
      """
      Page 3
      """

  Scenario: POST /forms/pdfengines/split (Intervals - pdfcpu)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_SPLIT_ENGINES | pdfcpu |
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3_0.pdf |
      | pages_3_1.pdf |
    Then the "pages_3_0.pdf" PDF should have 2 page(s)
    Then the "pages_3_1.pdf" PDF should have 1 page(s)

  @skip
  Scenario: POST /forms/pdfengines/split (Pages & Unify - PDFtk)
    # Reason: pdfbro uses lopdf/qpdf, not pdftk
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_SPLIT_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files      | testdata/pages_3.pdf | file  |
      | splitMode  | pages                | field |
      | splitSpan  | 2-end                | field |
      | splitUnify | true                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/split (Many PDFs - Lot of Pages)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_12.pdf | file  |
      | files     | testdata/pages_3.pdf  | file  |
      | splitMode | intervals             | field |
      | splitSpan | 1                     | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 15 PDF(s) in the response

  Scenario: POST /forms/pdfengines/split (Bad Request)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | foo                  | field |
      | splitSpan | 2                    | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Invalid form data: form field 'splitMode' is invalid (got 'foo', resulting to wrong value, expected either 'intervals' or 'pages')
      """
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | foo                  | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Invalid form data: form field 'splitSpan' is invalid (got 'foo', resulting to strconv.Atoi: parsing "foo": invalid syntax)
      """
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
      | pdfua     | foo                  | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Invalid form data: form field 'pdfua' is invalid (got 'foo', resulting to strconv.ParseBool: parsing "foo": invalid syntax)
      """

  @convert
  Scenario: POST /forms/pdfengines/split (PDF/A-1b & PDF/UA-1)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
      | pdfa      | PDF/A-1b             | field |
      | pdfua     | true                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3_0.pdf |
      | pages_3_1.pdf |
    Then the response PDF(s) should pass PDF/A validation

  @metadata
  Scenario: POST /forms/pdfengines/split (Metadata)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf                                                                                                                                                                                                                                                                                      | file  |
      | splitMode | intervals                                                                                                                                                                                                                                                                                                 | field |
      | splitSpan | 2                                                                                                                                                                                                                                                                                                         | field |
      | metadata  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3_0.pdf |
      | pages_3_1.pdf |

  @flatten
  Scenario: POST /forms/pdfengines/split (Flatten)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
      | flatten   | true                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  @encrypt
  Scenario: POST /forms/pdfengines/split (Encrypt - user password only)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files        | testdata/pages_3.pdf | file  |
      | splitMode    | intervals            | field |
      | splitSpan    | 2                    | field |
      | userPassword | foo                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @watermark
  Scenario: POST /forms/pdfengines/split (Watermark - Text)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files               | testdata/pages_3.pdf | file  |
      | splitMode           | intervals            | field |
      | splitSpan           | 2                    | field |
      | watermarkSource     | text                 | field |
      | watermarkExpression | CONFIDENTIAL         | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  @stamp
  Scenario: POST /forms/pdfengines/split (Stamp - Text)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files           | testdata/pages_3.pdf | file  |
      | splitMode       | intervals            | field |
      | splitSpan       | 2                    | field |
      | stampSource     | text                 | field |
      | stampExpression | DRAFT                | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  @rotate
  Scenario: POST /forms/pdfengines/split (Rotate 90)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files       | testdata/pages_3.pdf | file  |
      | splitMode   | intervals            | field |
      | splitSpan   | 2                    | field |
      | rotateAngle | 90                   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  @skip
  @embed
  Scenario: POST /forms/pdfengines/split (Embeds)
    # Reason: Embed file check step not yet implemented
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | embeds    | testdata/embed_1.xml | file  |
      | embeds    | testdata/embed_2.xml | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response

  @skip
  Scenario: POST /forms/pdfengines/split (Routes Disabled)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/split (Gotenberg Trace)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files           | testdata/pages_3.pdf   | file   |
      | splitMode       | intervals              | field  |
      | splitSpan       | 2                      | field  |
      | Gotenberg-Trace | forms_pdfengines_split | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_split"

  @output-filename
  Scenario: POST /forms/pdfengines/split (Output Filename - Single PDF)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files                     | testdata/pages_3.pdf | file   |
      | splitMode                 | pages                | field  |
      | splitSpan                 | 2-                   | field  |
      | splitUnify                | true                 | field  |
      | Gotenberg-Output-Filename | foo                  | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be the following file(s) in the response:
      | foo.pdf |

  @output-filename
  Scenario: POST /forms/pdfengines/split (Output Filename - Many PDFs)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files                     | testdata/pages_3.pdf | file   |
      | splitMode                 | intervals            | field  |
      | splitSpan                 | 2                    | field  |
      | Gotenberg-Output-Filename | foo                  | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be the following file(s) in the response:
      | foo.zip       |
      | pages_3_0.pdf |
      | pages_3_1.pdf |

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/split (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/pages_3.pdf","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
      | splitMode    | intervals                                                                                              | field |
      | splitSpan    | 2                                                                                                      | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/split (Webhook)
    # Reason: pdfbro uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files                       | testdata/pages_3.pdf                | file   |
      | splitMode                   | intervals                           | field  |
      | splitSpan                   | 2                                   | field  |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/split (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/split (Root Path)
    # Reason: pdfbro does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/split" with the following form data and header(s):
      | files     | testdata/pages_3.pdf | file  |
      | splitMode | intervals            | field |
      | splitSpan | 2                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"

  @convert
  @encrypt
  Scenario: POST /forms/pdfengines/split (PDF/A + Encrypt => 400)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files        | testdata/pages_3.pdf | file  |
      | splitMode    | intervals            | field |
      | splitSpan    | 2                    | field |
      | pdfa         | PDF/A-1b             | field |
      | userPassword | secret               | field |
    Then the response status code should be 400

  Scenario: POST /forms/pdfengines/split (stampSource=pdf without uploaded stamp file => 400)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files           | testdata/pages_3.pdf | file  |
      | splitMode       | intervals            | field |
      | splitSpan       | 2                    | field |
      | stampSource     | pdf                  | field |
      | stampExpression | /etc/hostname        | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a stamp file is required for image or pdf source
      """

  Scenario: POST /forms/pdfengines/split (watermarkSource=pdf without uploaded watermark file => 400)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files               | testdata/pages_3.pdf | file  |
      | splitMode           | intervals            | field |
      | splitSpan           | 2                    | field |
      | watermarkSource     | pdf                  | field |
      | watermarkExpression | /etc/hostname        | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a watermark file is required for image or pdf source
      """

  Scenario: POST /forms/pdfengines/split (Long Filename)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/split" with the following form data and header(s):
      | files                     | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.pdf | file   |
      | splitMode                 | pages                                                                                                                                                                                                                 | field  |
      | splitSpan                 | 1                                                                                                                                                                                                                     | field  |
      | splitUnify                | true                                                                                                                                                                                                                  | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
