import { createFileRoute, Link } from '@tanstack/react-router';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { blogSource } from '@/lib/source';

export const Route = createFileRoute('/blog/')({
  component: BlogIndex,
});

function BlogIndex() {
  const posts = blogSource.getPages();

  return (
    <HomeLayout {...baseOptions()}>
      <main className="container mx-auto px-4 py-12 max-w-4xl">
        <div className="mb-12">
          <h1 className="text-4xl font-bold mb-4">Pace Blog</h1>
          <p className="text-xl text-muted-foreground">
            News, releases, and articles about the Pace programming language.
          </p>
        </div>
        
        <div className="flex flex-col gap-6">
          {posts.map((post) => (
            <Link
              key={post.url}
              to={post.url}
              className="group block p-6 border rounded-xl hover:border-primary transition-colors bg-card"
            >
              <h2 className="text-2xl font-semibold mb-2 group-hover:text-primary transition-colors">
                {post.data.title}
              </h2>
              <p className="text-muted-foreground mb-4">
                {post.data.description}
              </p>
              {(post.data as any)?.date && (
                <div className="text-sm text-muted-foreground font-medium">
                  {new Date((post.data as any).date).toLocaleDateString(undefined, {
                    year: 'numeric',
                    month: 'long',
                    day: 'numeric'
                  })}
                </div>
              )}
            </Link>
          ))}
          {posts.length === 0 && (
            <p className="text-muted-foreground">No blog posts found.</p>
          )}
        </div>
      </main>
    </HomeLayout>
  );
}
