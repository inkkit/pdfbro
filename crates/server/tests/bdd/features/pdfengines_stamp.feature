# Feature: PDF Stamp
# Ported from Gotenberg's pdfengines_stamp.feature

Feature: /forms/pdfengines/stamp

  Scenario: POST /forms/pdfengines/stamp (text)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/stamp" with the following form data and header(s):
      | files | page_1.pdf | file  |
      | stamp | APPROVED   | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response
