'use client';

import { Sun, Moon, Monitor } from 'lucide-react';
import { useTheme, Theme } from '@/hooks/useTheme';

export default function ThemeToggle() {
    const { theme, setTheme, resolvedTheme } = useTheme();

    const cycleTheme = () => {
        const themes: Theme[] = ['light', 'dark', 'system'];
        const currentIndex = themes.indexOf(theme);
        const nextIndex = (currentIndex + 1) % themes.length;
        setTheme(themes[nextIndex]);
    };

    // Hydration note:
    // ThemeProvider initializes with deterministic values (`system`/`light`) on server and first client render,
    // then applies persisted/system preference after mount. Rendering directly here avoids an extra mount-only
    // state update that previously caused an unnecessary cascading render.
    return (
        <button
            onClick={cycleTheme}
            className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors text-gray-600 dark:text-gray-400"
            title={`Current theme: ${theme}. Click to change.`}
            aria-label="Toggle theme"
        >
            {theme === 'system' ? (
                <Monitor className="w-5 h-5" />
            ) : resolvedTheme === 'dark' ? (
                <Moon className="w-5 h-5" />
            ) : (
                <Sun className="w-5 h-5" />
            )}
        </button>
    );
}
