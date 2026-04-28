# Feature: PDF Encryption
# Ported from Gotenberg's pdfengines_encrypt.feature

Feature: /forms/pdfengines/encrypt

  Scenario: POST /forms/pdfengines/encrypt (user password)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files         | page_1.pdf | file  |
      | userPassword  | secret123  | field |
    Then the response status code should be 200
    Then the response header "Content-Type" should be "application/pdf"
    Then there should be 1 PDF(s) in the response

  Scenario: POST /forms/pdfengines/encrypt (owner password)
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/encrypt" with the following form data and header(s):
      | files         | page_1.pdf | file  |
      | ownerPassword | owner123   | field |
    Then the response status code should be 200

  Scenario: POST /forms/pdfengines/decrypt
    Given I have a default Folio container
    When I make a "POST" request to "/forms/pdfengines/decrypt" with the following form data and header(s):
      | files    | encrypted_page_1.pdf | file  |
      | password | secret123            | field |
    Then the response status code should be 200
