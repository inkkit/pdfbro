@pdfengines
@pdfengines-merge
@merge
Feature: /forms/pdfengines/merge

  Scenario: POST /forms/pdfengines/merge (default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | files                     | testdata/page_2.pdf | file   |
      | Gotenberg-Output-Filename | foo                 | header |
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

  Scenario: POST /forms/pdfengines/merge (QPDF)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_MERGE_ENGINES | qpdf |
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | files                     | testdata/page_2.pdf | file   |
      | Gotenberg-Output-Filename | foo                 | header |
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

  @folio-skip
  Scenario: POST /forms/pdfengines/merge (PDFtk)
    # Reason: Folio uses lopdf/qpdf, not pdftk
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_MERGE_ENGINES | pdftk |
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | files                     | testdata/page_2.pdf | file   |
      | Gotenberg-Output-Filename | foo                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 2 page(s)

  Scenario: POST /forms/pdfengines/merge (Bad Request)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Invalid form data: no form file found for extensions: [.pdf]
      """
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | files | testdata/page_2.pdf | file  |
      | pdfa  | foo                 | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files | testdata/page_1.pdf | file  |
      | files | testdata/page_2.pdf | file  |
      | pdfua | foo                 | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Invalid form data: form field 'pdfua' is invalid (got 'foo', resulting to strconv.ParseBool: parsing "foo": invalid syntax)
      """
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files    | testdata/page_1.pdf | file  |
      | files    | testdata/page_2.pdf | file  |
      | metadata | foo                 | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Invalid form data: form field 'metadata' is invalid (got 'foo', resulting to unmarshal metadata: invalid character 'o' in literal false (expecting 'a'))
      """

  @convert
  Scenario: POST /forms/pdfengines/merge (PDF/A-1b & PDF/UA-1)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | files                     | testdata/page_2.pdf | file   |
      | pdfa                      | PDF/A-1b            | field  |
      | pdfua                     | true                | field  |
      | Gotenberg-Output-Filename | foo                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 2 page(s)
    Then the response PDF(s) should pass PDF/A validation

  @metadata
  Scenario: POST /forms/pdfengines/merge (Metadata)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file   |
      | files                     | testdata/page_2.pdf                                                                                                                                                                                                                                                                                       | file   |
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 2 page(s)
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

  @bookmarks
  Scenario: POST /forms/pdfengines/merge (Bookmarks List)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                 | file   |
      | files                     | testdata/page_2.pdf                 | file   |
      | bookmarks                 | [{"title":"Merged Index","page":1}] | field  |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | teststore/foo.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "foo.pdf": [
          {
            "title": "Merged Index",
            "page": 1
          }
        ]
      }
      """

  @bookmarks
  Scenario: POST /forms/pdfengines/merge (Auto-index Bookmarks)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1_with_bookmarks.pdf | file   |
      | files                     | testdata/page_2_with_bookmarks.pdf | file   |
      | autoIndexBookmarks        | true                               | field  |
      | Gotenberg-Output-Filename | foo                                | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | teststore/foo.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "foo.pdf": [
          {
            "title": "Page 1",
            "page": 1
          },
          {
            "title": "Page 2",
            "page": 2
          }
        ]
      }
      """

  @flatten
  Scenario: POST /forms/pdfengines/merge (Flatten)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf | file   |
      | files                     | testdata/page_2.pdf | file   |
      | flatten                   | true                | field  |
      | Gotenberg-Output-Filename | foo                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 2 page(s)

  @encrypt
  Scenario: POST /forms/pdfengines/merge (Encrypt - user password only)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | files        | testdata/page_2.pdf | file  |
      | userPassword | foo                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @encrypt
  Scenario: POST /forms/pdfengines/merge (Encrypt - both user and owner passwords)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files         | testdata/page_1.pdf | file  |
      | files         | testdata/page_2.pdf | file  |
      | userPassword  | foo                 | field |
      | ownerPassword | bar                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @watermark
  Scenario: POST /forms/pdfengines/merge (Watermark - Text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files               | testdata/page_1.pdf | file  |
      | files               | testdata/page_2.pdf | file  |
      | watermarkSource     | text                | field |
      | watermarkExpression | CONFIDENTIAL        | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @stamp
  Scenario: POST /forms/pdfengines/merge (Stamp - Text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | files           | testdata/page_2.pdf | file  |
      | stampSource     | text                | field |
      | stampExpression | DRAFT               | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @rotate
  Scenario: POST /forms/pdfengines/merge (Rotate 90)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files       | testdata/page_1.pdf | file  |
      | files       | testdata/page_2.pdf | file  |
      | rotateAngle | 90                  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @folio-skip
  @embed
  Scenario: POST /forms/pdfengines/merge (Embeds)
    # Reason: Embed file check step not yet implemented
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/page_1.pdf  | file   |
      | files                     | testdata/page_2.pdf  | file   |
      | embeds                    | testdata/embed_1.xml | file   |
      | embeds                    | testdata/embed_2.xml | file   |
      | Gotenberg-Output-Filename | foo                  | header |
    Then the response status code should be 200

  Scenario: POST /forms/pdfengines/merge (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
      | files | testdata/page_2.pdf | file |
    Then the response status code should be 404

  Scenario: POST /forms/pdfengines/merge (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files           | testdata/page_1.pdf    | file   |
      | files           | testdata/page_2.pdf    | file   |
      | Gotenberg-Trace | forms_pdfengines_merge | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_merge"

  @folio-skip
  @download-from
  Scenario: POST /forms/pdfengines/merge (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf"},{"url":"http://host.docker.internal/static/testdata/page_2.pdf"}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @folio-skip
  @webhook
  Scenario: POST /forms/pdfengines/merge (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | files                       | testdata/page_2.pdf                 | file   |
      | Gotenberg-Output-Filename   | foo                                 | header |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  Scenario: POST /forms/pdfengines/merge (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
      | files | testdata/page_2.pdf | file |
    Then the response status code should be 401

  @folio-skip
  Scenario: POST /foo/forms/pdfengines/merge (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/merge" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
      | files | testdata/page_2.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @convert
  @encrypt
  Scenario: POST /forms/pdfengines/merge (PDF/A + Encrypt => 400)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files        | testdata/page_1.pdf | file  |
      | files        | testdata/page_2.pdf | file  |
      | pdfa         | PDF/A-1b            | field |
      | userPassword | secret              | field |
    Then the response status code should be 400

  Scenario: POST /forms/pdfengines/merge (stampSource=pdf without uploaded stamp file => 400)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files           | testdata/page_1.pdf | file  |
      | files           | testdata/page_2.pdf | file  |
      | stampSource     | pdf                 | field |
      | stampExpression | /etc/hostname       | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a stamp file is required for image or pdf source
      """

  Scenario: POST /forms/pdfengines/merge (watermarkSource=pdf without uploaded watermark file => 400)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files               | testdata/page_1.pdf | file  |
      | files               | testdata/page_2.pdf | file  |
      | watermarkSource     | pdf                 | field |
      | watermarkExpression | /etc/hostname       | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a watermark file is required for image or pdf source
      """

  Scenario: POST /forms/pdfengines/merge (Long Filename)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.pdf | file   |
      | files                     | testdata/page_2.pdf                                                                                                                                                                                                   | file   |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
