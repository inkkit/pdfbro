@pdfengines
@pdfengines-metadata
@metadata
Feature: /forms/pdfengines/metadata/{write|read}

  @skip
  Scenario: POST /forms/pdfengines/metadata/{write|read} (Single PDF)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file   |
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
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
  Scenario: POST /forms/pdfengines/metadata/{write|read} (Many PDFs)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files    | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file  |
      | files    | testdata/page_2.pdf                                                                                                                                                                                                                                                                                       | file  |
      | metadata | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | files | teststore/page_1.pdf | file |
      | files | teststore/page_2.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "page_1.pdf": {
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
        },
        "page_2.pdf": {
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

  Scenario: POST /forms/pdfengines/metadata/write (Bad Request)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files    | testdata/page_1.pdf | file  |
      | metadata | foo                 | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"

  Scenario: POST /forms/pdfengines/metadata/write (Reject Newline-Injected Pseudo-Tag)
    # Regression: a newline in a metadata value would split go-exiftool's
    # stdin line and inject an arbitrary ExifTool pseudo-tag such as
    # -FileName=, -SymLink=, or -HardLink=, allowing arbitrary filesystem
    # writes as the container user. WriteMetadata now rejects values
    # containing control characters with HTTP 400.
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files    | testdata/page_1.pdf                            | file  |
      | metadata | {"Title":"test\n-FileName=/tmp/inject_proof"} | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should contain string:
      """
      At least one PDF engine cannot process the requested metadata
      """

  Scenario: POST /forms/pdfengines/metadata/write (Reject Group-Prefixed Dangerous Tag)
    # Regression: ExifTool treats "System:FileName" identically to "FileName".
    # The dangerous-tag blocklist must strip group prefixes before comparing,
    # otherwise the attacker renames/moves files with a single HTTP request.
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                                                         | file   |
      | metadata                  | {"System:FileName":"stolen.pdf","System:Directory":"/tmp","Author":"legit"} | field  |
      | Gotenberg-Output-Filename | foo                                                                         | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | files | teststore/foo.pdf | file |
    Then the response status code should be 200
    Then the response body should contain string:
      """
      "Author":"legit"
      """

  Scenario: POST /forms/pdfengines/metadata/read (Bad Request)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      incomplete multipart stream
      """

  @skip
  Scenario: POST /forms/pdfengines/metadata/write (Routes Disabled)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files    | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file  |
      | metadata | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/metadata/read (Routes Disabled)
    Given I have a pdfbro container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/metadata/write (Gotenberg Trace)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files           | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file   |
      | metadata        | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Trace | forms_pdfengines_metadata_write                                                                                                                                                                                                                                                                           | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_metadata_write"

  @skip
  Scenario: POST /forms/pdfengines/metadata/read (Gotenberg Trace)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | files           | testdata/page_1.pdf            | file   |
      | Gotenberg-Trace | forms_pdfengines_metadata_read | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_metadata_read"

  @output-filename
  @skip
  Scenario: POST /forms/pdfengines/metadata/write (Output Filename - Single PDF)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file   |
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be the following file(s) in the response:
      | foo.pdf |

  @output-filename
  @skip
  Scenario: POST /forms/pdfengines/metadata/write (Output Filename - Many PDFs)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file   |
      | files                     | testdata/page_2.pdf                                                                                                                                                                                                                                                                                       | file   |
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be the following file(s) in the response:
      | foo.zip    |
      | page_1.pdf |
      | page_2.pdf |

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/metadata/write (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | downloadFrom              | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf","extraHttpHeaders":{"X-Foo":"bar"}}]                                                                                                                                                                                                    | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/metadata/read (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/metadata/write (Webhook)
    # Reason: pdfbro uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file   |
      | metadata                    | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 204

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/metadata/read (Webhook)
    # Reason: pdfbro uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/metadata/write (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files    | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file  |
      | metadata | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field |
    Then the response status code should be 401

  @skip
  @skip
  Scenario: POST /forms/pdfengines/metadata/read (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/metadata/{write|read} (Root Path)
    # Reason: pdfbro does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                                                                                                                                                                                                                                                                                       | file   |
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  Scenario: POST /forms/pdfengines/metadata/read (Long Filename)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/read" with the following form data and header(s):
      | files | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.pdf | file |
    Then the response status code should be 200

  Scenario: POST /forms/pdfengines/metadata/write (Long Filename)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/pdfengines/metadata/write" with the following form data and header(s):
      | files                     | testdata/Longitudinell_jämförelse_mellan_laserkirurgi_och_strålbehandling_gällande_röstkvalitet_och_självskattad_kommunikation_upp_till_två_år_efter_tidig_stämbandscancer_i_ett_randomiserat_kontrollerat_försök.pdf | file   |
      | metadata                  | {"Author":"Test"}                                                                                                                                                                                                     | field  |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
