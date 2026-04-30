@chromium
@chromium-screenshot-url
Feature: /forms/chromium/screenshot/url

  @skip
  Scenario: POST /forms/chromium/screenshot/url (Default)
    Given I have a default Folio container
    Given I have a static server
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url                       | http://host.docker.internal:%d/html/testdata/page-1-html/index.html | field  |
      | Gotenberg-Output-Filename | foo                                                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/png"
    Then there should be the following file(s) in the response:
      | foo.png |

  @skip
  Scenario: POST /forms/chromium/screenshot/url (JPEG)
    Given I have a default Folio container
    Given I have a static server
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url                       | http://host.docker.internal:%d/html/testdata/page-1-html/index.html | field  |
      | format                    | jpeg                                                                | field  |
      | Gotenberg-Output-Filename | foo                                                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/jpeg"
    Then there should be the following file(s) in the response:
      | foo.jpeg |

  @skip
  Scenario: POST /forms/chromium/screenshot/url (WebP)
    Given I have a default Folio container
    Given I have a static server
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url                       | http://host.docker.internal:%d/html/testdata/page-1-html/index.html | field  |
      | format                    | webp                                                                | field  |
      | Gotenberg-Output-Filename | foo                                                                 | header |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/webp"
    Then there should be the following file(s) in the response:
      | foo.webp |

  Scenario: POST /forms/chromium/screenshot/url (Bad Request - Missing URL)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | Gotenberg-Output-Filename | foo | header |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"

  Scenario: POST /forms/chromium/screenshot/url (file:// scheme rejected at route layer)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url | file:///tmp/foo/index.html | field |
    Then the response status code should be 400
    Then the response header "Content-Type" should be "application/json"
    Then the response body should match string:
      """
      file:// URLs are not accepted on this route. Use the /convert/html or /convert/markdown routes to render local HTML
      """

  @skip
  @webhook
  @skip
  Scenario: POST /forms/chromium/screenshot/url (Webhook)
    # Reason: Folio uses synchronous response API; no push webhook support
    Given I have a default Folio container
    Given I have a static server
    Given I have a webhook server
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url                         | http://host.docker.internal:%d/html/testdata/page-1-html/index.html | field  |
      | Gotenberg-Output-Filename   | foo                                                                 | header |
      | Gotenberg-Webhook-Url       | http://host.docker.internal:%d/webhook                              | header |
      | Gotenberg-Webhook-Error-Url | http://host.docker.internal:%d/webhook/error                        | header |
    Then the response status code should be 204
    When I wait for the asynchronous request to the webhook
    Then the webhook request header "Content-Type" should be "image/png"
    Then there should be the following file(s) in the webhook request:
      | foo.png |

  @skip
  @skip
  Scenario: POST /forms/chromium/screenshot/url (Basic Auth)
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_BASIC_AUTH             | true |
      | GOTENBERG_API_BASIC_AUTH_USERNAME | foo  |
      | GOTENBERG_API_BASIC_AUTH_PASSWORD | bar  |
    Given I have a static server
    When I make a "POST" request to "/forms/chromium/screenshot/url" with the following form data and header(s):
      | url | http://host.docker.internal:%d/html/testdata/page-1-html/index.html | field |
    Then the response status code should be 401

  @skip
  @skip
  Scenario: POST /foo/forms/chromium/screenshot/url (Root Path)
    # Reason: Folio does not support configurable API root path prefix
    Given I have a Folio container with the following environment variable(s):
      | API_ENABLE_DEBUG_ROUTE | true  |
      | API_ROOT_PATH          | /foo/ |
    Given I have a static server
    When I make a "POST" request to "/foo/forms/chromium/screenshot/url" with the following form data and header(s):
      | url | http://host.docker.internal:%d/html/testdata/page-1-html/index.html | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "image/png"
