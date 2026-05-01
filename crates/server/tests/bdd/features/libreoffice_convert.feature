@libreoffice
@libreoffice-convert
Feature: /forms/libreoffice/convert

  @skip
  Scenario: POST /forms/libreoffice/convert (Single Document)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/page_1.docx | file   |
      | Gotenberg-Output-Filename | foo                  | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """

  @skip
  Scenario: POST /forms/libreoffice/convert (Many Documents)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/page_1.docx | file   |
      | files                     | testdata/page_2.docx | file   |
      | Gotenberg-Output-Filename | foo                  | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.zip         |
      | page_1.docx.pdf |
      | page_2.docx.pdf |
    Then the "page_1.docx.pdf" PDF should have 1 page(s)
    Then the "page_2.docx.pdf" PDF should have 1 page(s)
    Then the "page_1.docx.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "page_2.docx.pdf" PDF should have the following content at page 1:
      """
      Page 2
      """

  @skip
  Scenario: POST /forms/libreoffice/convert (Non-basic Latin Characters)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/Special_Chars_ß.docx | file   |
      | Gotenberg-Output-Filename | foo                           | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """

  @skip
  Scenario: POST /forms/libreoffice/convert (Protected)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/protected_page_1.docx | file |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files    | testdata/protected_page_1.docx | file  |
      | password | foo                            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  Scenario: POST /forms/libreoffice/convert (Landscape)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/page_1.docx | file   |
      | Gotenberg-Output-Filename | foo                  | header |
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should NOT be set to landscape orientation
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/page_1.docx | file   |
      | landscape                 | true                 | field  |
      | Gotenberg-Output-Filename | foo                  | header |
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should be set to landscape orientation

  @skip
  Scenario: POST /forms/libreoffice/convert (Native Page Ranges - Single Document)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/pages_3.docx | file   |
      | nativePageRanges          | 2-3                   | field  |
      | Gotenberg-Output-Filename | foo                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 2 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Page 2
      """
    Then the "foo.pdf" PDF should have the following content at page 2:
      """
      Page 3
      """

  @skip
  Scenario: POST /forms/libreoffice/convert (Native Page Ranges - Many Documents)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/pages_3.docx  | file   |
      | files                     | testdata/pages_12.docx | file   |
      | nativePageRanges          | 2-3                    | field  |
      | Gotenberg-Output-Filename | foo                    | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.zip           |
      | pages_3.docx.pdf  |
      | pages_12.docx.pdf |
    Then the "pages_3.docx.pdf" PDF should have 2 page(s)
    Then the "pages_12.docx.pdf" PDF should have 2 page(s)

  Scenario: POST /forms/libreoffice/convert (Bad Request)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | landscape | foo | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files            | testdata/page_1.docx | file  |
      | nativePageRanges | foo                  | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/page_1.docx | file  |
      | files | testdata/page_2.docx | file  |
      | merge | foo                  | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Invalid form data: form field 'merge' is invalid (got 'foo', resulting to strconv.ParseBool: parsing "foo": invalid syntax)
      """

  @skip
  @merge
  Scenario: POST /forms/libreoffice/convert (Merge)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/page_1.docx | file   |
      | files                     | testdata/page_2.docx | file   |
      | merge                     | true                 | field  |
      | Gotenberg-Output-Filename | foo                  | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 2 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "foo.pdf" PDF should have the following content at page 2:
      """
      Page 2
      """

  @merge
  @split
  @skip
  Scenario: POST /forms/libreoffice/convert (Merge & Split)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files     | testdata/page_1.docx | file  |
      | files     | testdata/page_2.docx | file  |
      | merge     | true                 | field |
      | splitMode | intervals            | field |
      | splitSpan | 1                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | *_0.pdf |
      | *_1.pdf |
    Then the "*_0.pdf" PDF should have 1 page(s)
    Then the "*_1.pdf" PDF should have 1 page(s)
    Then the "*_0.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "*_1.pdf" PDF should have the following content at page 1:
      """
      Page 2
      """

  @split
  @skip
  Scenario: POST /forms/libreoffice/convert (Split Intervals)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files     | testdata/pages_3.docx | file  |
      | splitMode | intervals             | field |
      | splitSpan | 2                     | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3.docx_0.pdf |
      | pages_3.docx_1.pdf |
    Then the "pages_3.docx_0.pdf" PDF should have 2 page(s)
    Then the "pages_3.docx_1.pdf" PDF should have 1 page(s)

  @split
  @skip
  Scenario: POST /forms/libreoffice/convert (Split Pages)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files     | testdata/pages_3.docx | file  |
      | splitMode | pages                 | field |
      | splitSpan | 2-                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3.docx_0.pdf |
      | pages_3.docx_1.pdf |

  @split
  @skip
  Scenario: POST /forms/libreoffice/convert (Split Pages & Unify)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files      | testdata/pages_3.docx | file  |
      | splitMode  | pages                 | field |
      | splitSpan  | 2-                    | field |
      | splitUnify | true                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | pages_3.docx.pdf |
    Then the "pages_3.docx.pdf" PDF should have 2 page(s)

  @skip
  @convert
  Scenario: POST /forms/libreoffice/convert (PDF/A-1b & PDF/UA-1)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/page_1.docx | file  |
      | pdfa  | PDF/A-1b             | field |
      | pdfua | true                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  @skip
  @metadata
  Scenario: POST /forms/libreoffice/convert (Metadata)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/page_1.docx                                                                                                                                                                                                                                                                                      | file   |
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | files | teststore/foo.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "foo.pdf": {
          "Author": "Julien Neuhart",
          "Copyright": "Julien Neuhart",
          "CreateDate": "2006:09:18 16:27:50-04:00",
          "Creator": "Gotenberg",
          "Keywords": ["first", "second"],
          "Marked": true,
          "ModDate": "2006:09:18 16:27:50-04:00",
          "PDFVersion": 1.7,
          "Producer": "Gotenberg",
          "Subject": "Sample",
          "Title": "Sample",
          "Trapped": "Unknown"
        }
      }
      """

  @skip
  @flatten
  Scenario: POST /forms/libreoffice/convert (Flatten)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files   | testdata/page_1.docx | file  |
      | flatten | true                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @encrypt
  Scenario: POST /forms/libreoffice/convert (Encrypt - user password only)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files        | testdata/page_1.docx | file  |
      | userPassword | foo                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @skip
  @encrypt
  Scenario: POST /forms/libreoffice/convert (Encrypt - both user and owner passwords)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files         | testdata/page_1.docx | file  |
      | userPassword  | foo                  | field |
      | ownerPassword | bar                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @skip
  @watermark
  Scenario: POST /forms/libreoffice/convert (Watermark - Text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files               | testdata/page_1.docx | file  |
      | watermarkSource     | text                 | field |
      | watermarkExpression | CONFIDENTIAL         | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @stamp
  Scenario: POST /forms/libreoffice/convert (Stamp - Text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files           | testdata/page_1.docx | file  |
      | stampSource     | text                 | field |
      | stampExpression | DRAFT                | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @rotate
  Scenario: POST /forms/libreoffice/convert (Rotate 90)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files       | testdata/page_1.docx | file  |
      | rotateAngle | 90                   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @watermark
  Scenario: POST /forms/libreoffice/convert (Native Watermark - Text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files               | testdata/page_1.docx | file  |
      | nativeWatermarkText | CONFIDENTIAL         | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @watermark
  Scenario: POST /forms/libreoffice/convert (Native Watermark - Tiled)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                    | testdata/page_1.docx | file  |
      | nativeTiledWatermarkText | CONFIDENTIAL         | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @skip
  @embed
  Scenario: POST /forms/libreoffice/convert (Embeds)
    # Reason: Embed file check step not yet implemented
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/page_1.docx | file   |
      | embeds                    | testdata/embed_1.xml | file   |
      | embeds                    | testdata/embed_2.xml | file   |
      | Gotenberg-Output-Filename | foo                  | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |

  @skip
  Scenario: POST /forms/libreoffice/convert (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | LIBREOFFICE_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/page_1.docx | file |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/libreoffice/convert (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files           | testdata/page_1.docx      | file   |
      | Gotenberg-Trace | forms_libreoffice_convert | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_libreoffice_convert"

  @skip
  @download-from
  @skip
  Scenario: POST /forms/libreoffice/convert (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_1.docx","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/libreoffice/convert (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                       | testdata/page_1.docx                | file   |
      | Gotenberg-Output-Filename   | foo                                 | header |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/libreoffice/convert (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/page_1.docx | file |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/libreoffice/convert (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/libreoffice/convert" with the following form data and header(s):
      | files | testdata/page_1.docx | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @skip
  Scenario: POST /forms/libreoffice/convert (stampSource=pdf without uploaded stamp file => 400)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files           | testdata/page_1.docx | file  |
      | stampSource     | pdf                  | field |
      | stampExpression | /etc/hostname        | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a stamp file is required for image or pdf source
      """

  @skip
  Scenario: POST /forms/libreoffice/convert (watermarkSource=pdf without uploaded watermark file => 400)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files               | testdata/page_1.docx | file  |
      | watermarkSource     | pdf                  | field |
      | watermarkExpression | /etc/hostname        | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a watermark file is required for image or pdf source
      """

  @skip
  Scenario: POST /forms/libreoffice/convert (Long Filename)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.docx | file   |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                    | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "foo.pdf" PDF should have 1 page(s)
