import { createFileRoute, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { blogDocs, blogSource } from '@/lib/source';
import {
  DocsBody,
  DocsTitle,
} from 'fumadocs-ui/layouts/docs/page';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { encodeMarkdownUrl } from '@/lib/shared';
import { staticFunctionMiddleware } from '@tanstack/start-static-server-functions';
import { useFumadocsLoader } from 'fumadocs-core/source/client';
import { Suspense, use } from 'react';
import { useMDXComponents } from '@/components/mdx';

export const Route = createFileRoute('/blog/$')({
  component: Page,
  loader: async ({ params }) => {
    const slugs = params._splat?.split('/') ?? [];
    const data = await loader({ data: slugs });
    await blogDocs.getPage(data.path)?.preload();
    return data;
  },
});

const loader = createServerFn({
  method: 'GET',
})
  .validator((slugs: string[]) => slugs)
  .middleware([staticFunctionMiddleware])
  .handler(async ({ data: slugs }) => {
    const page = blogSource.getPage(slugs);
    if (!page) throw notFound();

    return {
      path: page.path,
      markdownUrl: encodeMarkdownUrl(page.slugs, page.locale),
      // For blog posts we don't necessarily need the whole tree unless we want to show a sidebar
      // We will just pass empty tree to DocsLayout, or pass the blogSource tree.
      pageTree: await blogSource.serializePageTree(blogSource.getPageTree()),
    };
  });

function Content({ path }: { path: string }) {
  const page = blogDocs.getPage(path);
  if (!page) throw new Error(`unknown page: ${path}`);

  const { toc } = use(page.load());
  const MDX = page.body;

  return (
    <main className="container mx-auto px-4 py-12 max-w-3xl">
      <div className="mb-12">
        <Link to="/blog" className="text-primary hover:underline mb-8 inline-block font-medium">
          &larr; Back to Blog
        </Link>
        <h1 className="text-4xl md:text-5xl font-bold mb-4">{page.title}</h1>
        <p className="text-xl text-muted-foreground mb-6">
          {page.description}
        </p>
        {(page as any).data?.date && (
          <div className="text-sm text-muted-foreground font-medium">
            {new Date((page as any).data.date).toLocaleDateString(undefined, {
              year: 'numeric',
              month: 'long',
              day: 'numeric'
            })}
          </div>
        )}
      </div>
      <DocsBody>
        <MDX components={useMDXComponents()} />
      </DocsBody>
    </main>
  );
}

function Page() {
  const { pageTree, path, markdownUrl } = useFumadocsLoader(Route.useLoaderData());

  return (
    <HomeLayout {...baseOptions()}>
      <Link to={markdownUrl} hidden />
      <Suspense>
        <Content path={path} />
      </Suspense>
    </HomeLayout>
  );
}
