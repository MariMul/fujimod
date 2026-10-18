"""Public binding checks use synthetic code, never camera code or disassembly."""
from pathlib import Path
import struct
import unittest
from fujimod.corpus import Index


class CorpusBinding(unittest.TestCase):
    def test_queries_roundtrip_merge_and_errors(self):
        indexes=[]
        body=struct.pack('<5I',0xe3a00001,0xe2800002,0xe2800003,0xe2800004,0xe12fff1e)
        for fw in ('a','b'):
            i=Index()
            i.add_unit(dict(firmware=fw,unit='bank',architecture='arm32',address_ranges=[],
                            functions=[dict(address=0x1000,ranges=[[0x1000,0x1014]],names=['synthetic'] if fw=='a' else [])]),[(0x1000,body)])
            i.build();indexes.append(i)
        indexes[0].merge(indexes[1]);i=indexes[0]
        f=i.find('synthetic',firmware='b')[0]
        self.assertEqual(i.names(f['id'],['a'])['synthetic'][0]['firmware'],'a')
        self.assertEqual(i.pairs('a','b')[0]['grade'],'identical')
        self.assertEqual(len(i.members(f['family'])),2)
        self.assertEqual(i.calls(f['id']),[])
        self.assertEqual(i.data_references(f['id']),[])
        self.assertEqual(i.references_to('b',0x1000),[])
        self.assertEqual(i.search('b',body[:4],unit='bank')[0]['address'],0x1000)
        with self.assertRaises(ValueError):i.search('b',b'')
        with self.assertRaises(ValueError):i.search('b',body,mask=b'x')
        i.annotate(f['family'],'note','synthetic','test')
        output=Path(__file__).resolve().parents[3]/'target/corpus-tests/python.index'
        output.parent.mkdir(parents=True,exist_ok=True)
        try:
            i.save(output);restored=Index.open(output)
            self.assertEqual(restored.family(f['family'])['annotations'][0]['text'],'synthetic')
        finally:
            output.unlink(missing_ok=True)

if __name__=='__main__':unittest.main()
