@chromium
@chromium-screenshot-markdown
Feature: /forms/chromium/screenshot/markdown

  Scenario: POST /forms/chromium/screenshot/markdown (Default)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/png"
    Then there should be the following file(s) in the response:
      | foo.png |

  Scenario: POST /forms/chromium/screenshot/markdown (JPEG)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | format                    | jpeg                                | field  |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/jpeg"
    Then there should be the following file(s) in the response:
      | foo.jpeg |

  Scenario: POST /forms/chromium/screenshot/markdown (WebP)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files                     | testdata/page-1-markdown/index.html | file   |
      | files                     | testdata/page-1-markdown/page_1.md  | file   |
      | format                    | webp                                | field  |
      | Gotenberg-Output-Filename | foo                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/webp"
    Then there should be the following file(s) in the response:
      | foo.webp |

  Scenario: POST /forms/chromium/screenshot/markdown (Bad Request - Missing Files)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"

  @skip
  @webhook
  @skip
  Scenario: POST /forms/chromium/screenshot/markdown (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    Given I have a webhook server
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files                       | testdata/page-1-markdown/index.html          | file   |
      | files                       | testdata/page-1-markdown/page_1.md           | file   |
      | Gotenberg-Output-Filename   | foo                                          | header |
      | Gotenberg-Webhook-Url       | http://host.docker.internal:%d/webhook       | header |
      | Gotenberg-Webhook-Error-Url | http://host.docker.internal:%d/webhook/error | header |
    Then the response status code should be 204
    When I wait for the asynchronous request to the webhook
    Then the webhook request header "Content-Type" should be "image/png"
    Then there should be the following file(s) in the webhook request:
      | foo.png |

  @skip
  @skip
  Scenario: POST /forms/chromium/screenshot/markdown (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    When I make a "POST" request to "/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files | testdata/page-1-markdown/index.html | file |
      | files | testdata/page-1-markdown/page_1.md  | file |
    Then the response status code should be 401

  @skip
  Scenario: POST /foo/forms/chromium/screenshot/markdown (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    When I make a "POST" request to "/foo/forms/chromium/screenshot/markdown" with the following form data and header(s):
      | files | testdata/page-1-markdown/index.html | file |
      | files | testdata/page-1-markdown/page_1.md  | file |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/png"
