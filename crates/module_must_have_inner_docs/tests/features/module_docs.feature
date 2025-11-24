Feature: Module inner documentation
  The lint ensures every module begins with an inner doc comment so the
  module's intent is immediately visible to readers.

  Scenario: Module begins with documentation
    Given the module begins with an inner doc comment
    When I validate the module documentation requirements
    Then the module is accepted

  Scenario: Module provides no inner documentation
    Given the module body starts with code only
    When I validate the module documentation requirements
    Then documentation is reported missing

  Scenario: Documentation follows other inner attributes
    Given the module contains an inner configuration attribute
    And documentation follows that attribute
    When I validate the module documentation requirements
    Then documentation is reported after other attributes

  Scenario: Only outer documentation exists
    Given the module declares only outer documentation
    When I validate the module documentation requirements
    Then documentation is reported missing
