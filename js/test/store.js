/* global describe, it */

const { Store } = require('../pkg/oxigraph.js')
const assert = require('assert')
const dataFactory = require('@rdfjs/data-model')

const ex = dataFactory.namedNode('http://example.com')
const triple = dataFactory.triple(
  dataFactory.blankNode('s'),
  dataFactory.namedNode('http://example.com/p'),
  dataFactory.literal('o')
)

describe('Store', function () {
  describe('#add()', function () {
    it('an added quad should be in the store', function () {
      const store = new Store()
      store.add(dataFactory.triple(ex, ex, triple))
      assert(store.has(dataFactory.triple(ex, ex, triple)))
    })
  })

  describe('#delete()', function () {
    it('an removed quad should not be in the store anymore', function () {
      const store = new Store([dataFactory.triple(triple, ex, ex)])
      assert(store.has(dataFactory.triple(triple, ex, ex)))
      store.delete(dataFactory.triple(triple, ex, ex))
      assert(!store.has(dataFactory.triple(triple, ex, ex)))
    })
  })

  describe('#has()', function () {
    it('an added quad should be in the store', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      assert(store.has(dataFactory.triple(ex, ex, ex)))
    })
  })

  describe('#size()', function () {
    it('A store with one quad should have 1 for size', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      assert.strictEqual(1, store.size)
    })
  })

  describe('#match_quads()', function () {
    it('blank pattern should return all quads', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      const results = store.match()
      assert.strictEqual(1, results.length)
      assert(dataFactory.triple(ex, ex, ex).equals(results[0]))
    })
  })

  describe('#query()', function () {
    it('ASK true', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      assert.strictEqual(true, store.query('ASK { ?s ?s ?s }'))
    })

    it('ASK false', function () {
      const store = new Store()
      assert.strictEqual(false, store.query('ASK { FILTER(false)}'))
    })

    it('CONSTRUCT', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      const results = store.query('CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }')
      assert.strictEqual(1, results.length)
      assert(dataFactory.triple(ex, ex, ex).equals(results[0]))
    })

    it('SELECT', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      const results = store.query('SELECT ?s WHERE { ?s ?p ?o }')
      assert.strictEqual(1, results.length)
      assert(ex.equals(results[0].get('s')))
    })

    it('SELECT with NOW()', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      const results = store.query('SELECT (YEAR(NOW()) AS ?y) WHERE {}')
      assert.strictEqual(1, results.length)
    })

    it('SELECT with RAND()', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      const results = store.query('SELECT (RAND() AS ?y) WHERE {}')
      assert.strictEqual(1, results.length)
    })
  })

  describe('#update()', function () {
    it('INSERT DATA', function () {
      const store = new Store()
      store.update('INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }')
      assert.strictEqual(1, store.size)
    })

    it('DELETE DATA', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      store.update('DELETE DATA { <http://example.com> <http://example.com> <http://example.com> }')
      assert.strictEqual(0, store.size)
    })

    it('DELETE WHERE', function () {
      const store = new Store([dataFactory.triple(ex, ex, ex)])
      store.update('DELETE WHERE { ?v ?v ?v }')
      assert.strictEqual(0, store.size)
    })
  })

  describe('#load()', function () {
    it('load NTriples in the default graph', function () {
      const store = new Store()
      store.load('<http://example.com> <http://example.com> <http://example.com> .', 'application/n-triples')
      assert(store.has(dataFactory.triple(ex, ex, ex)))
    })

    it('load NTriples in an other graph', function () {
      const store = new Store()
      store.load('<http://example.com> <http://example.com> <http://example.com> .', 'application/n-triples', null, ex)
      assert(store.has(dataFactory.quad(ex, ex, ex, ex)))
    })

    it('load Turtle with a base IRI', function () {
      const store = new Store()
      store.load('<http://example.com> <http://example.com> <> .', 'text/turtle', 'http://example.com')
      assert(store.has(dataFactory.triple(ex, ex, ex)))
    })

    it('load NQuads', function () {
      const store = new Store()
      store.load('<http://example.com> <http://example.com> <http://example.com> <http://example.com> .', 'application/n-quads')
      assert(store.has(dataFactory.quad(ex, ex, ex, ex)))
    })

    it('load TriG with a base IRI', function () {
      const store = new Store()
      store.load('GRAPH <> { <http://example.com> <http://example.com> <> }', 'application/trig', 'http://example.com')
      assert(store.has(dataFactory.quad(ex, ex, ex, ex)))
    })
  })

  describe('#dump()', function () {
    it('dump dataset content', function () {
      const store = new Store([dataFactory.quad(ex, ex, ex, ex)])
      assert.strictEqual('<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n', store.dump('application/n-quads'))
    })

    it('dump named graph content', function () {
      const store = new Store([dataFactory.quad(ex, ex, ex, ex)])
      assert.strictEqual('<http://example.com> <http://example.com> <http://example.com> .\n', store.dump('application/n-triples', ex))
    })

    it('dump default graph content', function () {
      const store = new Store([dataFactory.quad(ex, ex, ex, ex)])
      assert.strictEqual('', store.dump('application/n-triples'))
    })
  })
})
