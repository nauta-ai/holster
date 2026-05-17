from setuptools import Distribution, setup


class BinaryWheelDistribution(Distribution):
    """Force platform-specific wheels because they bundle holster-doctor."""

    def has_ext_modules(self) -> bool:
        return True


setup(distclass=BinaryWheelDistribution)
