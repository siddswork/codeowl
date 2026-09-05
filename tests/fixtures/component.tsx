import { useEffect, useState } from 'react';

/** Fetches and displays a user's display name. */
export function UserBadge({ userId }: { userId: string }): JSX.Element {
    const [name, setName] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        fetchUser(userId).then((user) => {
            if (!cancelled) setName(user.name);
        });
        return () => {
            cancelled = true;
        };
    }, [userId]);

    return <span>{name ?? 'Loading...'}</span>;
}

/** Picks the first item matching a predicate, or undefined. */
export function findFirst<T>(items: T[], predicate: (item: T) => boolean): T | undefined {
    return items.find(predicate);
}

async function fetchUser(userId: string): Promise<{ name: string }> {
    const res = await fetch(`/api/users/${userId}`);
    return res.json();
}
