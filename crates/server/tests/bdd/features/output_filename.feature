# Feature: Output Filename Header
# Ported from Gotenberg's output_filename.feature

Feature: Gotenberg-Output-Filename

  Scenario: Output filename for merge
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/merge" with the following form data and header(s):
      | files                     | page_1.pdf | file   |
      | files                     | page_2.pdf | file   |
      | Gotenberg-Output-Filename | mydocument | header |
    Then the response status code should be 200
    Then there should be the following file(s) in the response:
      | mydocument.pdf |

  Scenario: Output filename for convert
    Given I have a default Folio container
    When I make a "POST" request to "/forms/libreoffice/convert" with the following form data and header(s):
      | files                     | page_1.docx | file   |
      | Gotenberg-Output-Filename | report      | header |
    Then the response status code should be 200
