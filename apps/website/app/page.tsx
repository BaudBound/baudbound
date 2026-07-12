import { FeatureGrid } from "@/components/feature-grid";
import { Hero } from "@/components/hero";
import { HowItWorks } from "@/components/how-it-works";
import { ShowcaseSection } from "@/components/showcase-section";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { TrustSection } from "@/components/trust-section";

export default function Home() {
	return (
		<>
			<SiteHeader />
			<main>
				<Hero />
				<ShowcaseSection />
				<HowItWorks />
				<FeatureGrid />
				<TrustSection />
			</main>
			<SiteFooter />
		</>
	);
}
