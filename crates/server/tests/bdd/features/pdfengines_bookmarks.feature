@pdfengines
@pdfengines-bookmarks
@bookmarks
Feature: /forms/pdfengines/bookmarks/{write|read}

  Scenario: POST /forms/pdfengines/bookmarks/{write|read} (Single PDF & Bookmarks list)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                                                      | file   |
      | bookmarks                 | [{"title":"Index","page":1,"children":[{"title":"Sub-index","page":1}]}] | field  |
      | Gotenberg-Output-Filename | foo                                                                      | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | teststore/foo.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "foo.pdf": [
          {
            "title": "Index",
            "page": 1,
            "children": [
              {
                "title": "Sub-index",
                "page": 1
              }
            ]
          }
        ]
      }
      """

  Scenario: POST /forms/pdfengines/bookmarks/{write|read} (Single PDF & Bookmarks Map)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf                         | file   |
      | bookmarks                 | {"page_1.pdf":[{"title":"Index","page":1}]} | field  |
      | Gotenberg-Output-Filename | foo                                         | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | teststore/foo.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "foo.pdf": [
          {
            "title": "Index",
            "page": 1
          }
        ]
      }
      """

  Scenario: POST /forms/pdfengines/bookmarks/{write|read} (Many PDFs & Bookmarks List)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files     | testdata/page_1.pdf          | file  |
      | files     | testdata/page_2.pdf          | file  |
      | bookmarks | [{"title":"Index","page":1}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | teststore/page_1.pdf | file |
      | files | teststore/page_2.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "page_1.pdf": [
          {
            "title": "Index",
            "page": 1
          }
        ],
        "page_2.pdf": [
          {
            "title": "Index",
            "page": 1
          }
        ]
      }
      """

  Scenario: POST /forms/pdfengines/bookmarks/{write|read} (Many PDFs & Bookmarks Map)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files     | testdata/page_1.pdf                                                                   | file  |
      | files     | testdata/page_2.pdf                                                                   | file  |
      | bookmarks | {"page_1.pdf":[{"title":"Index","page":1}],"page_2.pdf":[{"title":"Index","page":1}]} | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | teststore/page_1.pdf | file |
      | files | teststore/page_2.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "page_1.pdf": [
          {
            "title": "Index",
            "page": 1
          }
        ],
        "page_2.pdf": [
          {
            "title": "Index",
            "page": 1
          }
        ]
      }
      """

  Scenario: POST /forms/pdfengines/bookmarks/read (Empty List)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match JSON:
      """
      {
        "page_1.pdf": []
      }
      """

  Scenario: POST /forms/pdfengines/bookmarks/write (Bad Request)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files     | testdata/page_1.pdf | file  |
      | bookmarks | foo                 | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"

  Scenario: POST /forms/pdfengines/bookmarks/read (Bad Request)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Invalid form data: no form file found for extensions: [.pdf]
      """

  @skip
  Scenario: POST /forms/pdfengines/bookmarks/write (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files     | testdata/page_1.pdf          | file  |
      | bookmarks | [{"title":"Index","page":1}] | field |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/bookmarks/read (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | PDFENGINES_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/pdfengines/bookmarks/write (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files           | testdata/page_1.pdf              | file   |
      | bookmarks       | [{"title":"Index","page":1}]     | field  |
      | Gotenberg-Trace | forms_pdfengines_bookmarks_write | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_bookmarks_write"

  @skip
  Scenario: POST /forms/pdfengines/bookmarks/read (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files           | testdata/page_1.pdf             | file   |
      | Gotenberg-Trace | forms_pdfengines_bookmarks_read | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"
    Then the response header "Gotenberg-Trace" should be "forms_pdfengines_bookmarks_read"

  @output-filename
  Scenario: POST /forms/pdfengines/bookmarks/write (Output Filename - Single PDF)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf          | file   |
      | bookmarks                 | [{"title":"Index","page":1}] | field  |
      | Gotenberg-Output-Filename | foo                          | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be the following file(s) in the response:
      | foo.pdf |

  @output-filename
  Scenario: POST /forms/pdfengines/bookmarks/write (Output Filename - Many PDFs)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf          | file   |
      | files                     | testdata/page_2.pdf          | file   |
      | bookmarks                 | [{"title":"Index","page":1}] | field  |
      | Gotenberg-Output-Filename | foo                          | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be the following file(s) in the response:
      | foo.zip    |
      | page_1.pdf |
      | page_2.pdf |

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/bookmarks/write (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | bookmarks                 | [{"title":"Index","page":1}]                                                                             | field  |
      | downloadFrom              | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf","extraHttpHeaders":{"X-Foo":"bar"}}]    | field  |
      | Gotenberg-Output-Filename | foo                                                                                                      | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @skip
  @download-from
  @skip
  Scenario: POST /forms/pdfengines/bookmarks/read (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page_1.pdf","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/json"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/bookmarks/write (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | bookmarks                   | [{"title":"Index","page":1}]        | field  |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @webhook
  @skip
  Scenario: POST /forms/pdfengines/bookmarks/read (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/bookmarks/read" with the following form data and header(s):
      | files                       | testdata/page_1.pdf                 | file   |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/pdfengines/bookmarks/write (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files     | testdata/page_1.pdf          | file  |
      | bookmarks | [{"title":"Index","page":1}] | field |
    Then the response status code should be 401

  @skip
  @skip
  Scenario: POST /forms/pdfengines/bookmarks/read (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files | testdata/page_1.pdf | file |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/pdfengines/bookmarks/{write|read} (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/pdfengines/bookmarks/write" with the following form data and header(s):
      | files                     | testdata/page_1.pdf          | file   |
      | bookmarks                 | [{"title":"Index","page":1}] | field  |
      | Gotenberg-Output-Filename | foo                          | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
