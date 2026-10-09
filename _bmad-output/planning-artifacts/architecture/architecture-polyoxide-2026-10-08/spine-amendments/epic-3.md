# Proposed spine amendments: Epic 3

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A3-1: `RetryPolicy::decide` takes the loop's schedule (amends AD-8's hook signatures)

- **Proposed by:** Claude, as the user's delegate, 2026-10-09, during Epic 3's bundle F (Story 3.1).
- **Current rule (AD-8):** the spine fixes only the return type: `RetryPolicy::decide -> Decision { outcome: Done | Retry(wait) | Fail, hold: Option<Duration> }`.
- **Proposed rule:** `RetryPolicy::decide(&self, &ResponseMeta, &AttemptInfo, schedule: &RetryConfig) -> Decision`. The third argument is the loop's own `RetryConfig`.
- **Why:**
  - **AD-9 sizes core's 429 hold from the schedule.** The hold is `retry_delay(0)`, which is not the attempt's floor, so a policy cannot compute it from `AttemptInfo` alone.
  - **One schedule, not two.** Without the argument, each policy would carry its own copy of the client's `RetryConfig`, and a client configured with `with_retry_config` would hold by one schedule and retry by another.
  - **The floor stays the loop's.** The loop still sleeps `max(retry_delay(attempt), wait)`; the argument lets a policy size a hold, never shorten a retry.
- **Follow-up in the code:** none. Story 3.1 ships this signature; Stories 3.4 to 3.6 build their policies on it.

## A3-2: `Authenticator::sign` returns core's `ApiError` (amends AD-8's hook signatures)

- **Proposed by:** Claude, as the user's delegate, 2026-10-09, during Epic 3's bundle F (Story 3.1).
- **Current rule (AD-8):** `Authenticator::sign(&mut RequestParts { method, path, query, headers, body }, attempt)`, with no error type named.
- **Proposed rule:** `sign` returns `impl Future<Output = Result<(), ApiError>> + Send`. A signing failure ends the request before it is sent, and the loop returns the error as it is.
- **Why:**
  - **The loop returns `ApiError`.** Every other way an attempt can fail is already one, so a signing failure needs no conversion inside the loop.
  - **The hooks are core's.** An error type per venue would make the trait generic, and `DynAuthenticator` could no longer be one type the loop holds.
- **Risk:** Story 3.4's L1 signing (EIP-712 over an alloy signer) may need a richer error than `ApiError` carries today. If it does, that story either adds an `ApiError` variant for a signing failure (AD-16 allows variant changes in S1) or amends this signature again, recorded here.
