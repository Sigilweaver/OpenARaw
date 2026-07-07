import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'intro',
    'install',
    'quickstart',
    {
      type: 'category',
      label: 'Guide',
      collapsed: false,
      items: [
        'guide/reader',
        'guide/scan-data',
        'guide/instrument-families',
        'guide/mzml-export',
      ],
    },
    {
      type: 'category',
      label: 'Format Specification',
      link: { type: 'doc', id: 'format/overview' },
      items: [
        'format/overview',
        'format/msscan',
        'format/secondary-bins',
        'format/mspeak',
        'format/msprofile',
        'format/msmasscal',
        'format/known-limitations',
      ],
    },
    'changelog',
    'license',
  ],
};

export default sidebars;
