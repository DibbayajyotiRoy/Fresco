/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  experimental: {
    optimizePackageImports: ["lucide-react"],
  },
  images: {
    remotePatterns: [
      {
        protocol: "https",
        hostname: "picsum.photos",
      },
      // YouTube poster art for the facade video embeds. Serving it through
      // next/image gets us AVIF/WebP + a right-sized srcset instead of the
      // raw 1280x720 JPEG YouTube hands out.
      {
        protocol: "https",
        hostname: "i.ytimg.com",
      },
    ],
  },
};

export default nextConfig;
