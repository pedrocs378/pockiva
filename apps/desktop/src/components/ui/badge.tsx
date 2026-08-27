import type { ComponentProps } from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const badgeVariants = cva(
  'inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden whitespace-nowrap',
  {
    variants: {
      variant: {
        secondary: 'rounded-full bg-[var(--muted)] px-2.5 py-0.5 text-xs text-[var(--muted-foreground)]',
        outline: 'rounded-full border border-[var(--border)] px-2.5 py-0.5 text-xs text-[var(--foreground)]'
      }
    },
    defaultVariants: {
      variant: 'secondary'
    }
  }
)

type BadgeProps = ComponentProps<'span'> & VariantProps<typeof badgeVariants>

const Badge = ({ className, variant, ...props }: BadgeProps) => (
  <span
    data-slot="badge"
    data-variant={variant ?? 'secondary'}
    className={cn(badgeVariants({ variant }), className)}
    {...props}
  />
)

export type { BadgeProps }
export { Badge, badgeVariants }
