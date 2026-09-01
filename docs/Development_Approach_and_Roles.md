# Development Approach and Roles

## Participants

- Myself (pvillela)
- Claude Opus 5 -- the main AI
- Google Gemini (via Google search)
- DeepSeek V4 Pro
- Claude Fable 5

## What I did

I followed an iterative approach for most of the items below, starting with a very narrow scope and expanding it along the way. I started with one project for the estimation of peak power attributable to EV charging activity and another one for the reading of Green Button data. Then I merged the two projects and gradually expanded scope.

- Defined software scope, functionality, and user interaction model.
- Defined software architecture, including architecture goals and module structure.
- Wrote initial versions of all core functional logic.
- Defined library API.
- Provided extensive interactive coding direction to AI agents, including writing shell code and laying out the target code structure.
- Wrote most of the content of the key functional documents and some notes documents, heavily edited subsidiary functional documents, reviewed and edited other documents.
- Directed and iterated with AI in the creation of technical and subsidiary functional documents.
- Reviewed code and documentation diffs on a priority basis to identify deviation from software direction and standards; when needed, directly undertook corrective action or instructed the AI to do so.
- Directed and interacted with independent AI models to review in detail the work done by the main AI.
- Directed and interacted with AI in the fixing of defects identified by me or other AIs.

## What the AIs did

- Claude Opus 5
  - Wrote all code other than core functional logic and shell code I authored, including all I/O and UI code (very few direct changes by me), as well as extensive automated tests.
  - Fixed defects in my code.
  - Wrote subsidiary functional documents as well as technical code-related documents.
  - Wrote the documentation and code for the electrotechnical site model.
  - Implemented the changes from the Fable 5 review, after my detailed review of the proposed changes and interaction with Opus 5 to clarify the changes to be made.
- Google Gemini (via Google search)
  - Wrote two electrotechnical documents.
- DeepSeek V4 Pro
  - Performed a detailed review of code and code comments, identifying a number of fixes and areas for improvement.
  - Implemented the proposed changes after my detailed review and interaction with the model.
- Claude Fable 5
  - Performed another detailed review of code and code comments after the DeepSeek V4 review-fix round. Identified a number of additional fixes and areas for improvement (which were implemented under my direction by Opus 5).

