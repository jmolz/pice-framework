import OpenAI from 'openai';
import { z } from 'zod';
import { zodResponseFormat } from 'openai/helpers/zod';

const DesignChallenge = z.object({
  severity: z.enum(['critical', 'consider', 'acknowledged']),
  finding: z.string(),
});

const AdversarialResult = z.object({
  designChallenges: z.array(DesignChallenge),
  overallAssessment: z.string(),
  recommendsChanges: z.boolean(),
});

export type AdversarialResultType = z.infer<typeof AdversarialResult>;

export async function runAdversarialEvaluation(
  prompt: string,
  model: string,
  effort: string,
): Promise<AdversarialResultType> {
  const client = new OpenAI();

  const completion = await client.chat.completions.parse({
    model,
    messages: [
      {
        role: 'system',
        content:
          'You are an adversarial design reviewer. Challenge the approach, assumptions, and design decisions. Your job is to find weaknesses, not confirm quality.',
      },
      { role: 'user', content: prompt },
    ],
    reasoning_effort: effort as 'low' | 'medium' | 'high',
    response_format: zodResponseFormat(AdversarialResult, 'adversarial_review'),
  });

  const result = completion.choices[0]?.message?.parsed;
  if (!result) {
    throw new Error('Failed to parse adversarial evaluation result');
  }
  return result;
}
