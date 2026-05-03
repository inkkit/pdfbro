@chromium
@chromium-convert-markdown
Feature: /forms/chromium/convert/markdown

  Scenario: POST /forms/chromium/convert/markdown (Default)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | Gotenberg-Output-Filename | foo                                 | header |
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

  Scenario: POST /forms/chromium/convert/markdown (Single Page)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/pages-12-markdown/index.html | file   |
      | files                     | testdata/pages-12-markdown/page_1.md  | file   |
      | files                     | testdata/pages-12-markdown/page_2.md  | file   |
      | files                     | testdata/pages-12-markdown/page_3.md  | file   |
      | files                     | testdata/pages-12-markdown/page_4.md  | file   |
      | files                     | testdata/pages-12-markdown/page_5.md  | file   |
      | files                     | testdata/pages-12-markdown/page_6.md  | file   |
      | files                     | testdata/pages-12-markdown/page_7.md  | file   |
      | files                     | testdata/pages-12-markdown/page_8.md  | file   |
      | files                     | testdata/pages-12-markdown/page_9.md  | file   |
      | files                     | testdata/pages-12-markdown/page_10.md | file   |
      | files                     | testdata/pages-12-markdown/page_11.md | file   |
      | files                     | testdata/pages-12-markdown/page_12.md | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 12 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "foo.pdf" PDF should have the following content at page 12:
      """
      Page 12
      """
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/pages-12-markdown/index.html | file   |
      | files                     | testdata/pages-12-markdown/page_1.md  | file   |
      | files                     | testdata/pages-12-markdown/page_2.md  | file   |
      | files                     | testdata/pages-12-markdown/page_3.md  | file   |
      | files                     | testdata/pages-12-markdown/page_4.md  | file   |
      | files                     | testdata/pages-12-markdown/page_5.md  | file   |
      | files                     | testdata/pages-12-markdown/page_6.md  | file   |
      | files                     | testdata/pages-12-markdown/page_7.md  | file   |
      | files                     | testdata/pages-12-markdown/page_8.md  | file   |
      | files                     | testdata/pages-12-markdown/page_9.md  | file   |
      | files                     | testdata/pages-12-markdown/page_10.md | file   |
      | files                     | testdata/pages-12-markdown/page_11.md | file   |
      | files                     | testdata/pages-12-markdown/page_12.md | file   |
      | singlePage                | true                                  | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Page 12
      """

  Scenario: POST /forms/chromium/convert/markdown (Landscape)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should NOT be set to landscape orientation
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | landscape                 | true                                | field  |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should be set to landscape orientation

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Native Page Ranges)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/pages-12-markdown/index.html | file   |
      | files                     | testdata/pages-12-markdown/page_1.md  | file   |
      | files                     | testdata/pages-12-markdown/page_2.md  | file   |
      | files                     | testdata/pages-12-markdown/page_3.md  | file   |
      | files                     | testdata/pages-12-markdown/page_4.md  | file   |
      | files                     | testdata/pages-12-markdown/page_5.md  | file   |
      | files                     | testdata/pages-12-markdown/page_6.md  | file   |
      | files                     | testdata/pages-12-markdown/page_7.md  | file   |
      | files                     | testdata/pages-12-markdown/page_8.md  | file   |
      | files                     | testdata/pages-12-markdown/page_9.md  | file   |
      | files                     | testdata/pages-12-markdown/page_10.md | file   |
      | files                     | testdata/pages-12-markdown/page_11.md | file   |
      | files                     | testdata/pages-12-markdown/page_12.md | file   |
      | nativePageRanges          | 2-3                                   | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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

  Scenario: POST /forms/chromium/convert/markdown (Header & Footer)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/pages-12-markdown/index.html   | file   |
      | files                     | testdata/pages-12-markdown/page_1.md    | file   |
      | files                     | testdata/pages-12-markdown/page_2.md    | file   |
      | files                     | testdata/pages-12-markdown/page_3.md    | file   |
      | files                     | testdata/pages-12-markdown/page_4.md    | file   |
      | files                     | testdata/pages-12-markdown/page_5.md    | file   |
      | files                     | testdata/pages-12-markdown/page_6.md    | file   |
      | files                     | testdata/pages-12-markdown/page_7.md    | file   |
      | files                     | testdata/pages-12-markdown/page_8.md    | file   |
      | files                     | testdata/pages-12-markdown/page_9.md    | file   |
      | files                     | testdata/pages-12-markdown/page_10.md   | file   |
      | files                     | testdata/pages-12-markdown/page_11.md   | file   |
      | files                     | testdata/pages-12-markdown/page_12.md   | file   |
      | files                     | testdata/header-footer-html/header.html | file   |
      | files                     | testdata/header-footer-html/footer.html | file   |
      | Gotenberg-Output-Filename | foo                                     | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 12 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Pages 12
      """
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      1 of 12
      """
    Then the "foo.pdf" PDF should have the following content at page 12:
      """
      Pages 12
      """
    Then the "foo.pdf" PDF should have the following content at page 12:
      """
      12 of 12
      """

  Scenario: POST /forms/chromium/convert/markdown (Wait Delay)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Wait delay > 2 seconds or expression window globalVar === 'ready' returns true.
      """
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | waitDelay                 | 2.5s                                      | field  |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Wait delay > 2 seconds or expression window globalVar === 'ready' returns true.
      """

  Scenario: POST /forms/chromium/convert/markdown (Wait For Expression)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Wait delay > 2 seconds or expression window globalVar === 'ready' returns true.
      """
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | waitForExpression         | window.globalVar === 'ready'              | field  |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Wait delay > 2 seconds or expression window globalVar === 'ready' returns true.
      """

  Scenario: POST /forms/chromium/convert/markdown (Emulated Media Type)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Emulated media type is 'print'.
      """
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Emulated media type is 'screen'.
      """
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | emulatedMediaType         | screen                                    | field  |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Emulated media type is 'screen'.
      """
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Emulated media type is 'print'.
      """

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Main URL does NOT match allowed list)
    Given I have a pdfbro container with the following environment variable(s):
      | CHROMIUM_ALLOW_LIST | ^file:(?!//\\/tmp/).* |
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | testdata/feature-rich-markdown/index.html | file |
      | files | testdata/feature-rich-markdown/table.md   | file |
    Then the response status code should be 403
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Forbidden
      """

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Main URL does match denied list)
    Given I have a pdfbro container with the following environment variable(s):
      | CHROMIUM_ALLOW_LIST |                |
      | CHROMIUM_DENY_LIST  | ^file:///tmp.* |
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | testdata/feature-rich-markdown/index.html | file |
      | files | testdata/feature-rich-markdown/table.md   | file |
    Then the response status code should be 403
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      Forbidden
      """

  Scenario: POST /forms/chromium/convert/markdown (JavaScript Enabled)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      JavaScript is enabled.
      """

  @skip
  Scenario: POST /forms/chromium/convert/markdown (JavaScript Disabled)
    Given I have a pdfbro container with the following environment variable(s):
      | CHROMIUM_DISABLE_JAVASCRIPT | true |
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/feature-rich-markdown/index.html | file   |
      | files                     | testdata/feature-rich-markdown/table.md   | file   |
      | Gotenberg-Output-Filename | foo                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      JavaScript is enabled.
      """

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Fail On Resource HTTP Status Codes)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                         | testdata/feature-rich-markdown/index.html | file  |
      | files                         | testdata/feature-rich-markdown/table.md   | file  |
      | failOnResourceHttpStatusCodes | [499,599]                                 | field |
    Then the response status code should be 409

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Fail On Resource Loading Failed)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                       | testdata/feature-rich-markdown/index.html | file  |
      | files                       | testdata/feature-rich-markdown/table.md   | file  |
      | failOnResourceLoadingFailed | true                                      | field |
    Then the response status code should be 409
    Then the response header "Content-Type" should be "application/json"
    Then the response body should contain string:
      """
      Chromium failed to load resources
      """

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Fail On Console Exceptions)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                   | testdata/feature-rich-markdown/index.html | file  |
      | files                   | testdata/feature-rich-markdown/table.md   | file  |
      | failOnConsoleExceptions | true                                      | field |
    Then the response status code should be 409
    Then the response header "Content-Type" should be "application/json"
    Then the response body should contain string:
      """
      Chromium console exceptions
      """

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Bad Request)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | testdata/pages-3-markdown/index.html | file |
      | files | testdata/pages-3-markdown/page_1.md  | file |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files          | testdata/page-1-markdown/index.html | file  |
      | files          | testdata/page-1-markdown/page_1.md  | file  |
      | omitBackground | true                                | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files            | testdata/page-1-markdown/index.html | file  |
      | files            | testdata/page-1-markdown/page_1.md  | file  |
      | nativePageRanges | foo                                 | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files             | testdata/page-1-markdown/index.html | file  |
      | files             | testdata/page-1-markdown/page_1.md  | file  |
      | waitForExpression | undefined                           | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"

  @split
  @skip
  Scenario: POST /forms/chromium/convert/markdown (Split Intervals)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files     | testdata/pages-3-markdown/index.html | file  |
      | files     | testdata/pages-3-markdown/page_1.md  | file  |
      | files     | testdata/pages-3-markdown/page_2.md  | file  |
      | files     | testdata/pages-3-markdown/page_3.md  | file  |
      | splitMode | intervals                            | field |
      | splitSpan | 2                                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | *_0.pdf |
      | *_1.pdf |
    Then the "*_0.pdf" PDF should have 2 page(s)
    Then the "*_1.pdf" PDF should have 1 page(s)
    Then the "*_0.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "*_0.pdf" PDF should have the following content at page 2:
      """
      Page 2
      """
    Then the "*_1.pdf" PDF should have the following content at page 1:
      """
      Page 3
      """

  # See https://github.com/gotenberg/gotenberg/issues/1130.
  @split
  @output-filename
  @skip
  Scenario: POST /forms/chromium/convert/markdown (Split Output Filename)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/pages-3-markdown/index.html | file   |
      | files                     | testdata/pages-3-markdown/page_1.md  | file   |
      | files                     | testdata/pages-3-markdown/page_2.md  | file   |
      | files                     | testdata/pages-3-markdown/page_3.md  | file   |
      | splitMode                 | intervals                            | field  |
      | splitSpan                 | 2                                    | field  |
      | Gotenberg-Output-Filename | foo                                  | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.zip   |
      | foo_0.pdf |
      | foo_1.pdf |
    Then the "foo_0.pdf" PDF should have 2 page(s)
    Then the "foo_1.pdf" PDF should have 1 page(s)

  @split
  @skip
  Scenario: POST /forms/chromium/convert/markdown (Split Pages)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files     | testdata/pages-3-markdown/index.html | file  |
      | files     | testdata/pages-3-markdown/page_1.md  | file  |
      | files     | testdata/pages-3-markdown/page_2.md  | file  |
      | files     | testdata/pages-3-markdown/page_3.md  | file  |
      | splitMode | pages                                | field |
      | splitSpan | 2-                                   | field |
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
      Page 2
      """
    Then the "*_1.pdf" PDF should have the following content at page 1:
      """
      Page 3
      """

  @split
  @skip
  Scenario: POST /forms/chromium/convert/markdown (Split Pages & Unify)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/pages-3-markdown/index.html | file   |
      | files                     | testdata/pages-3-markdown/page_1.md  | file   |
      | files                     | testdata/pages-3-markdown/page_2.md  | file   |
      | files                     | testdata/pages-3-markdown/page_3.md  | file   |
      | splitMode                 | pages                                | field  |
      | splitSpan                 | 2-                                   | field  |
      | splitUnify                | true                                 | field  |
      | Gotenberg-Output-Filename | foo                                  | header |
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

  @convert
  Scenario: POST /forms/chromium/convert/markdown (PDF/A-1b & PDF/UA-1)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | testdata/page-1-markdown/index.html | file  |
      | files | testdata/page-1-markdown/page_1.md  | file  |
      | pdfa  | PDF/A-1b                            | field |
      | pdfua | true                                | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  @metadata
  Scenario: POST /forms/chromium/convert/markdown (Metadata)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html                                                                                                                                                                                                                                                                       | file   |
      | files                     | testdata/page-1-markdown/page_1.md                                                                                                                                                                                                                                                                        | file   |
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

  @flatten
  Scenario: POST /forms/chromium/convert/markdown (Flatten)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files   | testdata/page-1-markdown/index.html | file  |
      | files   | testdata/page-1-markdown/page_1.md  | file  |
      | flatten | true                                | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @encrypt
  Scenario: POST /forms/chromium/convert/markdown (Encrypt - user password only)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files        | testdata/page-1-markdown/index.html | file  |
      | files        | testdata/page-1-markdown/page_1.md  | file  |
      | userPassword | foo                                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @encrypt
  Scenario: POST /forms/chromium/convert/markdown (Encrypt - both user and owner passwords)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files         | testdata/page-1-markdown/index.html | file  |
      | files         | testdata/page-1-markdown/page_1.md  | file  |
      | userPassword  | foo                                 | field |
      | ownerPassword | bar                                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should be encrypted

  @skip
  @embed
  Scenario: POST /forms/chromium/convert/markdown (Embeds)
    # Reason: Embed file check step not yet implemented
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | embeds                    | testdata/embed_1.xml                | file   |
      | embeds                    | testdata/embed_2.xml                | file   |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Routes Disabled)
    Given I have a pdfbro container with the following environment variable(s):
      | CHROMIUM_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | testdata/page-1-markdown/index.html | file |
      | files | testdata/page-1-markdown/page_1.md  | file |
    Then the response status code should be 404

  @skip
  Scenario: POST /forms/chromium/convert/markdown (Gotenberg Trace)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files           | testdata/page-1-markdown/index.html | file   |
      | files           | testdata/page-1-markdown/page_1.md  | file   |
      | Gotenberg-Trace | forms_chromium_convert_markdown     | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_chromium_convert_markdown"

  @skip
  @download-from
  @skip
  Scenario: POST /forms/chromium/convert/markdown (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal/static/testdata/page-1-markdown/index.html","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
      | files        | testdata/page-1-markdown/page_1.md                                                                                    | file  |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/chromium/convert/markdown (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                       | testdata/page-1-markdown/index.html | file   |
      | files                       | testdata/page-1-markdown/page_1.md  | file   |
      | Gotenberg-Output-Filename   | foo                                 | header |
      | Gotenberg-Webhook-Url       | http://host.docker.internal/webhook | header |
    Then the response status code should be 204

  @skip
  @skip
  Scenario: POST /forms/chromium/convert/markdown (Basic Auth)
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | testdata/page-1-markdown/index.html | file |
      | files | testdata/page-1-markdown/page_1.md  | file |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/chromium/convert/markdown (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a pdfbro container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/chromium/convert/markdown" with the following form data and header(s):
      | files | testdata/page-1-markdown/index.html | file |
      | files | testdata/page-1-markdown/page_1.md  | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @skip
  Scenario: POST /forms/chromium/convert/markdown (stampSource=pdf without uploaded stamp file => 400)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files           | testdata/page-1-markdown/index.html | file  |
      | files           | testdata/page-1-markdown/page_1.md  | file  |
      | stampSource     | pdf                                 | field |
      | stampExpression | /etc/hostname                       | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a stamp file is required for image or pdf source
      """

  @skip
  Scenario: POST /forms/chromium/convert/markdown (watermarkSource=pdf without uploaded watermark file => 400)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files               | testdata/page-1-markdown/index.html | file  |
      | files               | testdata/page-1-markdown/page_1.md  | file  |
      | watermarkSource     | pdf                                 | field |
      | watermarkExpression | /etc/hostname                       | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a watermark file is required for image or pdf source
      """

  Scenario: POST /forms/chromium/convert/markdown (Long Filename)
    Given I have a default pdfbro container
    When I make a "POST" request to "/forms/chromium/convert/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "foo.pdf" PDF should have 1 page(s)
