import type { ComponentProps } from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const buttonVariants = cva(
  'inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap transition-colors outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0',
  {
    variants: {
      variant: {
        default: 'rounded-md bg-[var(--primary)] text-sm font-medium text-[var(--primary-foreground)]',
        secondary:
          'rounded-md border border-[var(--border)] bg-[var(--muted)] text-sm font-medium text-[var(--foreground)]',
        unstyled: ''
      },
      size: {
        default: 'h-9 px-4 py-2',
        sm: 'h-8 px-3',
        icon: 'size-9',
        auto: ''
      }
    },
    defaultVariants: {
      variant: 'default',
      size: 'default'
    }
  }
)

type ButtonProps = ComponentProps<'button'> & VariantProps<typeof buttonVariants>

const Button = ({ className, variant, size, ...props }: ButtonProps) => (
  <button
    data-slot="button"
    data-variant={variant ?? 'default'}
    data-size={size ?? 'default'}
    className={cn(buttonVariants({ variant, size }), className)}
    {...props}
  />
)

export type { ButtonProps }
export { Button, buttonVariants }
