@chromium
@chromium-convert-html
Feature: /forms/chromium/convert/html

  Scenario: POST /forms/chromium/convert/html (Default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/page-1-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                             | header |
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

  Scenario: POST /forms/chromium/convert/html (Single Page)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/pages-12-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                               | header |
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
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/pages-12-html/index.html | file   |
      | singlePage                | true                              | field  |
      | Gotenberg-Output-Filename | foo                               | header |
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
      # page-break-after: always; tells the browser's print engine to force a page break after each element,
      # even when calculating a large enough paper height, Chromium's PDF rendering will still honor those page break
      # directives.
      """
      Page 12
      """

  Scenario: POST /forms/chromium/convert/html (Landscape)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/page-1-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                             | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should NOT be set to landscape orientation
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/page-1-html/index.html | file   |
      | landscape                 | true                            | field  |
      | Gotenberg-Output-Filename | foo                             | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should be set to landscape orientation

  Scenario: POST /forms/chromium/convert/html (Native Page Ranges)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/pages-12-html/index.html | file   |
      | nativePageRanges          | 2-3                               | field  |
      | Gotenberg-Output-Filename | foo                               | header |
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

  Scenario: POST /forms/chromium/convert/html (Header & Footer)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/pages-12-html/index.html       | file   |
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

  Scenario: POST /forms/chromium/convert/html (Wait Delay)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
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
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | waitDelay                 | 2.5s                                  | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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

  Scenario: POST /forms/chromium/convert/html (Wait For Expression)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
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
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | waitForExpression         | window.globalVar === 'ready'          | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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

  Scenario: POST /forms/chromium/convert/html (rAF / ResizeObserver / IntersectionObserver fire with waitForExpression)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/paint-callbacks-html/index.html       | file   |
      | waitForExpression         | !!document.body.getAttribute('data-pdf-ready') | field  |
      | Gotenberg-Output-Filename | foo                                            | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      raf-fired
      """
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      ro-fired
      """
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      io-fired
      """

  Scenario: POST /forms/chromium/convert/html (rAF / ResizeObserver / IntersectionObserver fire with waitDelay and emulatedMediaType=print)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/paint-callbacks-html/index.html | file   |
      | waitDelay                 | 3s                                       | field  |
      | emulatedMediaType         | print                                    | field  |
      | Gotenberg-Output-Filename | foo                                      | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      raf-fired
      """
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      ro-fired
      """
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      io-fired
      """

  Scenario: POST /forms/chromium/convert/html (Wait For Selector)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Wait on selector returns true.
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | waitForSelector           | #wait-selector                        | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Wait on selector returns true.
      """

  Scenario: POST /forms/chromium/convert/html (Emulated Media Type)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
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
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | emulatedMediaType         | print                                 | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | emulatedMediaType         | screen                                | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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

  Scenario: POST /forms/chromium/convert/html (Emulated Media Features)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Prefers reduced motion.
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | emulatedMediaFeatures     | {"prefers-reduced-motion":"reduce"}   | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Prefers reduced motion.
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | emulatedMediaType         | screen                                | field  |
      | emulatedMediaFeatures     | {"prefers-reduced-motion":"reduce"}   | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Prefers reduced motion.
      """
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Emulated media type is 'print'.
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | emulatedMediaType         | print                                 | field  |
      | emulatedMediaFeatures     | {"prefers-reduced-motion":"reduce"}   | field  |
      | Gotenberg-Output-Filename | foo                                   | header |
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
    Then the "foo.pdf" PDF should have the following content at page 1:
      """
      Prefers reduced motion.
      """
    Then the "foo.pdf" PDF should NOT have the following content at page 1:
      """
      Emulated media type is 'screen'.
      """

  Scenario: POST /forms/chromium/convert/html (Default Allow / Deny Lists)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/feature-rich-html/index.html | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/chromium/convert/html (Main URL does NOT match allowed list)
    Given I have a Folio container with the following environment variable(s):
      | CHROMIUM_ALLOW_LIST | ^file:(?!//\\/tmp/).* |
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/feature-rich-html/index.html | file |
    Then the response status code should be 403
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Forbidden
      """

  Scenario: POST /forms/chromium/convert/html (Main URL does match denied list)
    Given I have a Folio container with the following environment variable(s):
      | CHROMIUM_ALLOW_LIST |                |
      | CHROMIUM_DENY_LIST  | ^file:///tmp.* |
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/feature-rich-html/index.html | file |
    Then the response status code should be 403
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Forbidden
      """

  Scenario: POST /forms/chromium/convert/html (Request does not match the allowed list)
    Given I have a Folio container with the following environment variable(s):
      | CHROMIUM_ALLOW_LIST | ^file:///tmp.* |
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/feature-rich-html/index.html | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/chromium/convert/html (JavaScript Enabled)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
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

  Scenario: POST /forms/chromium/convert/html (JavaScript Disabled)
    Given I have a Folio container with the following environment variable(s):
      | CHROMIUM_DISABLE_JAVASCRIPT | true |
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/feature-rich-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                                   | header |
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

  Scenario: POST /forms/chromium/convert/html (Fail On Resource HTTP Status Codes)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                         | testdata/feature-rich-html/index.html | file  |
      | failOnResourceHttpStatusCodes | [499,599]                             | field |
    Then the response status code should be 409
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Invalid HTTP status code from resources:
      https://gethttpstatus.com/400 - 400: Bad Request
      """

  Scenario: POST /forms/chromium/convert/html (Fail On Resource HTTP Status Codes - Ignore Domains)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                           | testdata/feature-rich-html/index.html | file  |
      | failOnResourceHttpStatusCodes   | [499,599]                             | field |
      | ignoreResourceHttpStatusDomains | ["gethttpstatus.com"]                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/chromium/convert/html (Fail On Resource Loading Failed)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                       | testdata/feature-rich-html/index.html | file  |
      | failOnResourceLoadingFailed | true                                  | field |
    Then the response status code should be 409
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should contain string:
      """
      Chromium failed to load resources
      """
    Then the response body should contain string:
      """
      resource Stylesheet: net::ERR_CONNECTION_REFUSED
      """
    Then the response body should contain string:
      """
      resource Stylesheet: net::ERR_FILE_NOT_FOUND
      """

  Scenario: POST /forms/chromium/convert/html (Fail On Console Exceptions)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                   | testdata/feature-rich-html/index.html | file  |
      | failOnConsoleExceptions | true                                  | field |
    Then the response status code should be 409
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should contain string:
      """
      Chromium console exceptions
      """
    Then the response body should contain string:
      """
      Error: Exception 1
      """
    Then the response body should contain string:
      """
      Error: Exception 2
      """

  Scenario: POST /forms/chromium/convert/html (Bad Request)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | singlePage                    | foo | field |
      | paperWidth                    | foo | field |
      | paperHeight                   | foo | field |
      | marginTop                     | foo | field |
      | marginBottom                  | foo | field |
      | marginLeft                    | foo | field |
      | marginRight                   | foo | field |
      | preferCssPageSize             | foo | field |
      | generateDocumentOutline       | foo | field |
      | generateTaggedPdf             | foo | field |
      | printBackground               | foo | field |
      | omitBackground                | foo | field |
      | landscape                     | foo | field |
      | scale                         | foo | field |
      | waitDelay                     | foo | field |
      | emulatedMediaType             | foo | field |
      | failOnHttpStatusCodes         | foo | field |
      | failOnResourceHttpStatusCodes | foo | field |
      | failOnResourceLoadingFailed   | foo | field |
      | failOnConsoleExceptions       | foo | field |
      | skipNetworkIdleEvent          | foo | field |
      | skipNetworkAlmostIdleEvent    | foo | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files          | testdata/page-1-html/index.html | file  |
      | omitBackground | true                            | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      omitBackground requires printBackground set to true
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files            | testdata/page-1-html/index.html | file  |
      | nativePageRanges | foo                             | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      Chromium does not handle the page ranges 'foo' (nativePageRanges) syntax
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files            | testdata/page-1-html/index.html | file  |
      | nativePageRanges | 2-3                             | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      The page ranges '2-3' (nativePageRanges) exceeds the page count
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files             | testdata/page-1-html/index.html | file  |
      | waitForExpression | undefined                       | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    Then the response body should match string:
      """
      The expression 'undefined' (waitForExpression) returned an exception or undefined
      """
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files   | testdata/page-1-html/index.html | file  |
      | cookies | foo                             | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files            | testdata/page-1-html/index.html | file  |
      | extraHttpHeaders | foo                             | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files     | testdata/page-1-html/index.html | file  |
      | splitMode | foo                             | field |
      | splitSpan | 2                               | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/page-1-html/index.html | file  |
      | pdfa  | foo                             | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/page-1-html/index.html | file  |
      | pdfua | foo                             | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files    | testdata/page-1-html/index.html | file  |
      | metadata | foo                             | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "text/plain; charset=UTF-8"

  @split
  Scenario: POST /forms/chromium/convert/html (Split Intervals)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files     | testdata/pages-3-html/index.html | file  |
      | splitMode | intervals                        | field |
      | splitSpan | 2                                | field |
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
  Scenario: POST /forms/chromium/convert/html (Split Output Filename)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/pages-3-html/index.html | file   |
      | splitMode                 | intervals                        | field  |
      | splitSpan                 | 2                                | field  |
      | Gotenberg-Output-Filename | foo                              | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.zip   |
      | foo_0.pdf |
      | foo_1.pdf |
    Then the "foo_0.pdf" PDF should have 2 page(s)
    Then the "foo_1.pdf" PDF should have 1 page(s)
    Then the "foo_0.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "foo_0.pdf" PDF should have the following content at page 2:
      """
      Page 2
      """
    Then the "foo_1.pdf" PDF should have the following content at page 1:
      """
      Page 3
      """

  @split
  Scenario: POST /forms/chromium/convert/html (Split Pages)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files     | testdata/pages-3-html/index.html | file  |
      | splitMode | pages                            | field |
      | splitSpan | 2-                               | field |
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
  Scenario: POST /forms/chromium/convert/html (Split Pages & Unify)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/pages-3-html/index.html | file   |
      | splitMode                 | pages                            | field  |
      | splitSpan                 | 2-                               | field  |
      | splitUnify                | true                             | field  |
      | Gotenberg-Output-Filename | foo                              | header |
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

  @split
  Scenario: POST /forms/chromium/convert/html (Split Many PDFs - Lot of Pages)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files     | testdata/pages-12-html/index.html | file  |
      | splitMode | intervals                         | field |
      | splitSpan | 1                                 | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 12 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | *_0.pdf  |
      | *_1.pdf  |
      | *_2.pdf  |
      | *_3.pdf  |
      | *_4.pdf  |
      | *_5.pdf  |
      | *_6.pdf  |
      | *_7.pdf  |
      | *_8.pdf  |
      | *_9.pdf  |
      | *_10.pdf |
      | *_11.pdf |
    Then the "*_0.pdf" PDF should have 1 page(s)
    Then the "*_11.pdf" PDF should have 1 page(s)
    Then the "*_0.pdf" PDF should have the following content at page 1:
      """
      Page 1
      """
    Then the "*_11.pdf" PDF should have the following content at page 1:
      """
      Page 12
      """

  @convert
  Scenario: POST /forms/chromium/convert/html (PDF/A-1b & PDF/UA-1)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/page-1-html/index.html | file  |
      | pdfa  | PDF/A-1b                        | field |
      | pdfua | true                            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the response PDF(s) should pass PDF/A validation

  @convert
  @split
  Scenario: POST /forms/chromium/convert/html (Split & PDF/A-1b & PDF/UA-1)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files     | testdata/pages-3-html/index.html | file  |
      | splitMode | intervals                        | field |
      | splitSpan | 2                                | field |
      | pdfa      | PDF/A-1b                         | field |
      | pdfua     | true                             | field |
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
  @convert
  @split
  @output-filename
  Scenario: POST /forms/chromium/convert/html (Split & PDF/A-1b & PDF/UA-1 & Output Filename)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/pages-3-html/index.html | file   |
      | splitMode                 | intervals                        | field  |
      | splitSpan                 | 2                                | field  |
      | pdfa                      | PDF/A-1b                         | field  |
      | pdfua                     | true                             | field  |
      | Gotenberg-Output-Filename | foo                              | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/zip"
    Then there should be 2 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.zip   |
      | foo_0.pdf |
      | foo_1.pdf |
    Then the "foo_0.pdf" PDF should have 2 page(s)
    Then the "foo_1.pdf" PDF should have 1 page(s)

  @metadata
  Scenario: POST /forms/chromium/convert/html (Metadata)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/page-1-html/index.html                                                                                                                                                                                                                                                                           | file   |
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
  Scenario: POST /forms/chromium/convert/html (Flatten)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files   | testdata/page-1-html/index.html | file  |
      | flatten | true                            | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @encrypt
  Scenario: POST /forms/chromium/convert/html (Encrypt - user password only)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files        | testdata/page-1-html/index.html | file  |
      | userPassword | foo                             | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @encrypt
  Scenario: POST /forms/chromium/convert/html (Encrypt - both user and owner passwords)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files         | testdata/page-1-html/index.html | file  |
      | userPassword  | foo                             | field |
      | ownerPassword | bar                             | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @watermark
  Scenario: POST /forms/chromium/convert/html (Watermark - Text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files               | testdata/page-1-html/index.html | file  |
      | watermarkSource     | text                            | field |
      | watermarkExpression | CONFIDENTIAL                    | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @stamp
  Scenario: POST /forms/chromium/convert/html (Stamp - Text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files           | testdata/page-1-html/index.html | file  |
      | stampSource     | text                            | field |
      | stampExpression | DRAFT                           | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @rotate
  Scenario: POST /forms/chromium/convert/html (Rotate 90)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files       | testdata/page-1-html/index.html | file  |
      | rotateAngle | 90                              | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  @folio-skip
  @embed
  Scenario: POST /forms/chromium/convert/html (Embeds)
    # Reason: Embed file check step not yet implemented
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/page-1-html/index.html | file   |
      | embeds                    | testdata/embed_1.xml            | file   |
      | embeds                    | testdata/embed_2.xml            | file   |
      | Gotenberg-Output-Filename | foo                             | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the response PDF(s) should have the "embed_1.xml" file embedded
    Then the response PDF(s) should have the "embed_2.xml" file embedded

  # FIXME: once decrypt is done, add encrypt and check after the content of the PDF.
  @folio-skip
  @convert
  @metadata
  @watermark
  @stamp
  @flatten
  @embed
  Scenario: POST /forms/chromium/convert/html (PDF/A-3b & PDF/UA-1 & Metadata & Watermark & Stamp & Flatten & Embeds)
    # Reason: Embed file check step not yet implemented
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/page-1-html/index.html                                                                                                                                                                                                                                                                           | file   |
      | pdfa                      | PDF/A-3b                                                                                                                                                                                                                                                                                                  | field  |
      | pdfua                     | true                                                                                                                                                                                                                                                                                                      | field  |
      | metadata                  | {"Author":"Julien Neuhart","Copyright":"Julien Neuhart","CreateDate":"2006-09-18T16:27:50-04:00","Creator":"Gotenberg","Keywords":["first","second"],"Marked":true,"ModDate":"2006-09-18T16:27:50-04:00","PDFVersion":1.7,"Producer":"Gotenberg","Subject":"Sample","Title":"Sample","Trapped":"Unknown"} | field  |
      | flatten                   | true                                                                                                                                                                                                                                                                                                      | field  |
      | embeds                    | testdata/embed_1.xml                                                                                                                                                                                                                                                                                      | file   |
      | embeds                    | testdata/embed_2.xml                                                                                                                                                                                                                                                                                      | file   |
      | Gotenberg-Output-Filename | foo                                                                                                                                                                                                                                                                                                       | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then there should be the following file(s) in the response:
      | foo.pdf |
    Then the response PDF(s) should have the "embed_1.xml" file embedded
    Then the response PDF(s) should have the "embed_2.xml" file embedded

  Scenario: POST /forms/chromium/convert/html (Routes Disabled)
    Given I have a Folio container with the following environment variable(s):
      | CHROMIUM_DISABLE_ROUTES | true |
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/page-1-html/index.html | file |
    Then the response status code should be 404

  Scenario: POST /forms/chromium/convert/html (Gotenberg Trace)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files           | testdata/page-1-html/index.html | file   |
      | Gotenberg-Trace | forms_chromium_convert_html     | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then the response header "Gotenberg-Trace" should be "forms_chromium_convert_html"

  @folio-skip
  @download-from
  Scenario: POST /forms/chromium/convert/html (Download From)
    # Reason: downloadFrom with live static server requires integration environment
    Given I have a default Folio container
    Given I have a static server
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | downloadFrom | [{"url":"http://host.docker.internal:%d/static/testdata/page-1-html/index.html","extraHttpHeaders":{"X-Foo":"bar"}}] | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  @folio-skip
  @webhook
  Scenario: POST /forms/chromium/convert/html (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    Given I have a webhook server
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                       | testdata/page-1-html/index.html              | file   |
      | Gotenberg-Output-Filename   | foo                                          | header |
      | Gotenberg-Webhook-Url       | http://host.docker.internal:%d/webhook       | header |
      | Gotenberg-Webhook-Error-Url | http://host.docker.internal:%d/webhook/error | header |
    Then the response status code should be 204
    When I wait for the asynchronous request to the webhook
    Then the webhook request header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the webhook request

  Scenario: POST /forms/chromium/convert/html (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/page-1-html/index.html | file |
    Then the response status code should be 401

  @folio-skip
  Scenario: POST /foo/forms/chromium/convert/html (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/chromium/convert/html" with the following form data and header(s):
      | files | testdata/page-1-html/index.html | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"

  # See: https://github.com/gotenberg/gotenberg/issues/1505.
  Scenario: POST /forms/chromium/convert/html (Asset)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/html-with-asset/index.html | file   |
      | files                     | testdata/html-with-asset/image.png  | file   |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "foo.pdf" PDF should have 1 page(s)
    Then the "foo.pdf" PDF should have 1 image(s)

  Scenario: POST /forms/chromium/convert/html (stampSource=pdf without uploaded stamp file => 400)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files           | testdata/page-1-html/index.html | file  |
      | stampSource     | pdf                             | field |
      | stampExpression | /etc/hostname                   | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a stamp file is required for image or pdf source
      """

  Scenario: POST /forms/chromium/convert/html (watermarkSource=pdf without uploaded watermark file => 400)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files               | testdata/page-1-html/index.html | file  |
      | watermarkSource     | pdf                             | field |
      | watermarkExpression | /etc/hostname                   | field |
    Then the response status code should be 400
    Then the response body should match string:
      """
      Invalid form data: a watermark file is required for image or pdf source
      """

  # See: https://github.com/gotenberg/gotenberg/issues/1500.
  Scenario: POST /forms/chromium/convert/html (Long Filename)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/convert/html" with the following form data and header(s):
      | files                     | testdata/page-1-html/index.html | file   |
      | Gotenberg-Output-Filename | foo                             | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
    Then the "foo.pdf" PDF should have 1 page(s)
