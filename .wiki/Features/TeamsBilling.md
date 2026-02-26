# Teams Billing

Teams billing enables organizations to share a ClawDE Cloud subscription across multiple users, with per-seat pricing and owner-managed access control.

## Pricing

Teams is priced per seat per month. The minimum is 2 seats. The owner manages the team from `base.clawde.io/teams/billing`.

Contact [support@clawde.io](mailto:support@clawde.io) for volume pricing.

## Setting Up a Team

1. Go to `base.clawde.io/teams/billing`
2. Enter your payment method
3. Choose your initial seat count
4. Click **Subscribe**

Stripe processes payment. The team is active immediately.

## Managing Members

**Inviting a member:**

1. Go to `base.clawde.io/teams/billing`
2. Click **+ Invite member**
3. Enter their email address
4. They receive an invitation email with a 7-day sign-up link

Once they accept, they are added to the team and can access ClawDE Cloud features.

**Removing a member:**

Click **Remove** next to any member. Their access is revoked immediately. You will receive a prorated credit on the next billing cycle.

## Seat Management

**Adding a seat:** Click **+ Add Seat**. Stripe prorates the charge and issues an immediate invoice for the partial month.

**Removing a seat:** Click **− Remove Seat**. The seat credit applies at the next billing cycle.

## Payment Failure

If payment fails:

1. Stripe retries automatically over 4 days
2. The team owner receives an email notification
3. If payment is not resolved within 3 days: team access is suspended (members cannot connect)
4. After resolution: access is restored automatically

## Team Access in the Web App

Members access team features at `app.clawde.io/team` — member list, roles, and pending invites. The owner can manage billing at `base.clawde.io/teams/billing`.

## Roles

| Role | Capabilities |
| --- | --- |
| Owner | Manage billing, invite/remove members, full API access |
| Member | Full ClawDE Cloud API access; cannot manage team or billing |
