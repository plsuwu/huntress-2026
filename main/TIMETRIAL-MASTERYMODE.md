# Time Trial and Mastery Mode

Here is the whole idea: our compounding challenges grow with you.

Single Solve is where you learn it. Time Trial is where you prove you can do it under pressure. Mastery Mode is where you show that you can do it repeatedly and reliably.

Complete Single Solve first to unlock Time Trial. Complete Time Trial to unlock Mastery Mode for that same challenge and round.

## Regular play and Run for Time

Check the challenge's Game Master `op=list` entry. If it says `"run_for_time_supported":true`, a normal start gives you a regular untimed challenge. Solve its assigned artifact and submit the answers as usual: a correct solve earns the flag and normal CourseStack points. You do not need a `practice` command.

When you want to race the clock, opt in with the JSON boolean "run_for_time":true:

```
{"op":"start","challenge":"<challenge-id>","run_for_time":true}
```

After a completed or expired attempt, use `op=retry` with the same boolean to get the next fresh timed variant. Every fresh qualified timed attempt can improve your fastest Hall of Fame result. Regular and timed attempts draw from separate artifacts. Repeating `start` always reconnects to the attempt you already have; it does not change its mode or spend another variant. Finish the current attempt before switching through retry.

The Game Master reports `ranked_attempts_remaining` and `regular_variants_remaining` separately. Once fresh regular variants run out, you can still retry for a flag and CourseStack credit on the last regular artifact; `"variant_fresh":false` makes that reuse clear. Timed ranking attempts never repeat an artifact for your team.

The instructions below also cover existing routes that do not advertise `run_for_time_supported`. Those routes are still timed by default and retain the older `practice` continuation until their separate regular and timed pools are qualified and released.

## Time Trial

On older routes, or when you choose `run_for_time:true` on a supported route, Time Trial puts one assigned challenge variant on a real, server-enforced clock. The timer does not begin while you are reading the lesson or waiting for the Game Master to prepare your material. It begins only when that material is ready and the timed attempt starts.

For a timed attempt, the Game Master includes the exact UTC deadline in the start or status response:

```
{"download":"<assigned-artifact>","deadline":"<UTC timestamp>","unique_attempts_remaining":2}
```

Once that clock is moving, it is moving. Repeating the start request resumes the same attempt. Closing Homebase, reconnecting, refreshing CourseStack, or asking a teammate to start the challenge does not give you a fresh timer.

## Mastery Mode

Mastery Mode turns the same idea up a notch. You will receive multiple tasks or variants. On a timed attempt they share one overall deadline; on a supported regular attempt, you can complete them without a timer.

Submit each finding under the exact answer ID returned by the Game Master. You do not have to submit everything at once: correct partial progress is saved, and the response tells you how many answers are still outstanding:

```
{"result":"accepted","remaining":2}
```

You finish Mastery Mode when every required answer has been accepted. For a timed attempt, they must all be accepted before the shared deadline. Repeating an answer that was already accepted will not advance your progress.

## Expiry, reconnect, and retry

These timer and expiry rules apply to older timed-by-default routes and to attempts started with `run_for_time:true`. Regular untimed attempts can continue until solved; `retry` starts a new regular variant after CourseStack confirms the solve.

- If you reconnect or repeat a normal start request, you return to the current attempt with its original deadline.
- You cannot reset a timer while it is still active.
- If time runs out before the solve is complete, the Game Master returns `{"result":"expired"}`. Late answers cannot earn a timed result on that attempt.
- After an attempt expires, an explicit retry starts one new shared timer, clears any partial answers from the expired run, and advances to the next qualified variant.
- After CourseStack confirms a completed attempt, an explicit retry advances to the next qualified variant. A normal start always reconnects to the same attempt. If scoring is still pending, that attempt stays in place until confirmation.
- A new attempt ID and artifact SHA-256 show that the variant advanced. The challenge ID, question IDs, answer formats, and scoring destination remain stable by design.
- `unique_attempts_remaining` shows how many fresh variants your team or solo playthrough has after the current one. Reconnects and duplicate retry requests do not spend another entry.
- Each challenge has its own published retry pool. A future timed route does not promise a new variant until its qualified rotation is published; the live Game Master response and `unique_attempts_remaining` are authoritative.
- If teammates retry at the same time, those requests converge on the same new team timer rather than creating separate attempts.
- Every newly issued, qualified timed variant can improve your team's fastest Hall of Fame time. The Game Master reports `hall_of_fame_eligible` for your current attempt. Older retries that were not marked eligible when they occurred remain historical; their times are not retroactively added to the Hall of Fame.

Use this explicit retry request after the attempt has expired or its solve has been confirmed:

```
{"op":"retry","challenge":"<challenge-id>"}
```

On older routes without `run_for_time_supported`, when the final fresh variant has ended, `retry` reports `rotation_exhausted`. You can keep working on its artifact without a timer or Hall of Fame time by starting a continuation:

```
{"op":"practice","challenge":"<challenge-id>"}
```

A correct submission in this older continuation can still complete this mode and unlock the next one. The Game Master marks it with `"practice":true`.

> **IMPORTANT**: The deadline returned by the Game Master is the authoritative clock. A browser refresh, local stopwatch, reconnect, or repeated start request cannot tell you that the server-side timer changed.

